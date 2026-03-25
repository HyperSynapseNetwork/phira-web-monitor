use anyhow::{Context, Result};
use log::{error, trace, warn};
use phira_mp_common::{
    ClientCommand, ServerCommand, Stream as MpStream, HEARTBEAT_INTERVAL, HEARTBEAT_TIMEOUT,
};
use std::{
    future::Future,
    ops::Deref,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    sync::{Mutex, Notify},
    task::JoinHandle,
    time,
};

pub trait MpClientState: Send + Sync + 'static {
    fn process(&self, cmd: ServerCommand) -> impl Future<Output = ()> + Send;
}

pub struct MpClient<S>
where
    S: MpClientState,
{
    state: Arc<S>,
    stream: Arc<MpStream<ClientCommand, ServerCommand>>,
    delay: Arc<Mutex<Option<Duration>>>,
    ping_notify: Arc<Notify>,
    ping_fail_count: Arc<AtomicU8>,
    ping_task_handle: JoinHandle<()>,
}

impl<S> MpClient<S>
where
    S: MpClientState,
{
    pub async fn new(mp_server: &str, state: S) -> Result<Self> {
        let tcp_stream = TcpStream::connect(mp_server).await?;
        tcp_stream.set_nodelay(true)?;

        let state = Arc::new(state);
        let delay = Arc::new(Mutex::default());
        let ping_notify = Arc::new(Notify::new());
        let ping_fail_count = Arc::new(AtomicU8::default());

        let stream = Arc::new(
            MpStream::new(
                Some(1),
                tcp_stream,
                Box::new({
                    let state = Arc::clone(&state);
                    let ping_notify = Arc::clone(&ping_notify);
                    move |_, cmd| {
                        let is_pong = matches!(cmd, ServerCommand::Pong);
                        if is_pong {
                            ping_notify.notify_one();
                        }
                        let state = Arc::clone(&state);
                        async move {
                            if !is_pong {
                                state.process(cmd).await;
                            }
                        }
                    }
                }),
            )
            .await?,
        );

        let ping_task_handle = tokio::spawn({
            let stream = Arc::clone(&stream);
            let ping_notify = Arc::clone(&ping_notify);
            let ping_fail_count = Arc::clone(&ping_fail_count);
            let delay = Arc::clone(&delay);
            async move {
                loop {
                    time::sleep(HEARTBEAT_INTERVAL).await;

                    let start = Instant::now();
                    if let Err(err) = stream.send(ClientCommand::Ping).await {
                        error!("failed to send heartbeat: {err:?}");
                    } else if time::timeout(HEARTBEAT_TIMEOUT, ping_notify.notified())
                        .await
                        .is_err()
                    {
                        warn!("heartbeat timeout");
                        ping_fail_count.fetch_add(1, Ordering::Relaxed);
                    } else {
                        ping_fail_count.store(0, Ordering::SeqCst);
                    }
                    let evaled_delay = start.elapsed();
                    *delay.lock().await = Some(evaled_delay);
                    trace!("sent heartbeat, delay: {evaled_delay:?}");
                }
            }
        });

        Ok(Self {
            state,
            stream,
            delay,
            ping_notify,
            ping_fail_count,
            ping_task_handle,
        })
    }

    pub fn state(&self) -> Arc<S> {
        Arc::clone(&self.state)
    }

    pub fn delay(&self) -> Option<Duration> {
        *self.delay.blocking_lock()
    }

    pub fn ping_fail_count(&self) -> u8 {
        self.ping_fail_count.load(Ordering::Relaxed)
    }

    pub async fn send(&self, cmd: ClientCommand) -> Result<()> {
        self.stream.send(cmd).await
    }

    pub fn blocking_send(&self, cmd: ClientCommand) -> Result<()> {
        self.stream.blocking_send(cmd)
    }

    pub async fn ping(&self) -> Result<Duration> {
        let start = Instant::now();
        self.send(ClientCommand::Ping).await?;
        time::timeout(HEARTBEAT_TIMEOUT, self.ping_notify.notified())
            .await
            .context("heartbeat timeout")?;
        let delay = start.elapsed();
        *self.delay.lock().await = Some(delay);
        Ok(delay)
    }
}

impl<S> Deref for MpClient<S>
where
    S: MpClientState,
{
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<S> Drop for MpClient<S>
where
    S: MpClientState,
{
    fn drop(&mut self) {
        self.ping_task_handle.abort();
    }
}
