use anyhow::{anyhow, Context, Error, Result};
use axum::response::sse::Event;
use futures::StreamExt;
use phira_mp_common::{
    generate_secret_key, ClientCommand, ClientRoomState, RoomId, ServerCommand, Stream, UserInfo,
    HEARTBEAT_INTERVAL, HEARTBEAT_TIMEOUT,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    convert::Infallible,
    future::Future,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    sync::{broadcast, oneshot, Mutex, Notify, RwLock},
    task::JoinHandle,
    time,
};
use tokio_stream::wrappers::BroadcastStream;

type SResult<T> = Result<T, String>;

const TIMEOUT: Duration = Duration::from_secs(3);

struct TaskResult<T> {
    lock: Mutex<()>,
    tx: Mutex<Option<oneshot::Sender<T>>>,
}

impl<T> TaskResult<T> {
    pub fn new() -> Self {
        TaskResult {
            lock: Mutex::default(),
            tx: Mutex::default(),
        }
    }
    pub async fn acquire<F>(&self, f: impl FnOnce() -> F) -> Result<T>
    where
        F: Future<Output = Result<()>>,
    {
        let _guard = self.lock.lock().await;
        let (tx, rx) = oneshot::channel();
        *self.tx.lock().await = Some(tx);
        f().await?;
        Ok(time::timeout(TIMEOUT, rx).await??)
    }
    pub async fn put(&self, value: T) -> Result<()> {
        self.tx
            .lock()
            .await
            .take()
            .ok_or_else(|| anyhow!("no active task"))?
            .send(value)
            .map_err(|_| anyhow!("failed to send value"))
    }
}

struct ClientState {
    delay: Mutex<Option<Duration>>,
    ping_notify: Notify,

    authenticate_result: TaskResult<SResult<(UserInfo, Option<ClientRoomState>)>>,
    room_result: TaskResult<SResult<(HashMap<RoomId, Value>, HashMap<i32, RoomId>)>>,

    /// (room state, update events, next sync time)
    cached_room_state: RwLock<(HashMap<RoomId, Value>, HashMap<i32, RoomId>)>,
    cached_events: RwLock<Vec<Event>>,
    next_sync_time: Mutex<Instant>,
    broadcast_tx: broadcast::Sender<Event>,
}

impl ClientState {
    pub async fn push_event(&self, event: Event) -> Result<()> {
        let mut events = self.cached_events.write().await;
        events.push(event.clone());
        self.broadcast_tx.send(event)?;
        Ok(())
    }
}

pub struct RoomMonitorClient {
    state: Arc<ClientState>,
    stream: Arc<Stream<ClientCommand, ServerCommand>>,

    ping_fail_count: Arc<AtomicU8>,
    ping_task_handle: JoinHandle<()>,
}

impl RoomMonitorClient {
    pub async fn new(mp_server: &str) -> Result<Self> {
        let tcp_stream = TcpStream::connect(mp_server).await?;
        tcp_stream.set_nodelay(true)?;

        let state = Arc::new(ClientState {
            delay: Mutex::default(),
            ping_notify: Notify::new(),

            authenticate_result: TaskResult::new(),
            room_result: TaskResult::new(),

            cached_room_state: RwLock::default(),
            cached_events: RwLock::default(),
            next_sync_time: Mutex::new(Instant::now()),

            broadcast_tx: broadcast::channel(1024).0,
        });
        let stream = Arc::new(
            Stream::new(
                Some(1),
                tcp_stream,
                Box::new({
                    let state = Arc::clone(&state);
                    move |_, cmd| process(Arc::clone(&state), cmd)
                }),
            )
            .await?,
        );

        let ping_fail_count = Arc::new(AtomicU8::default());
        let ping_task_handle = tokio::spawn({
            let stream = Arc::clone(&stream);
            let state = Arc::clone(&state);
            let ping_fail_count = Arc::clone(&ping_fail_count);
            async move {
                loop {
                    time::sleep(HEARTBEAT_INTERVAL).await;

                    let start = Instant::now();
                    if let Err(err) = stream.send(ClientCommand::Ping).await {
                        log::error!("failed to send heartbeat: {err:?}");
                    } else if time::timeout(HEARTBEAT_TIMEOUT, state.ping_notify.notified())
                        .await
                        .is_err()
                    {
                        log::warn!("heartbeat timeout");
                        ping_fail_count.fetch_add(1, Ordering::Relaxed);
                    } else {
                        ping_fail_count.store(0, Ordering::SeqCst);
                    }
                    let delay = start.elapsed();
                    *state.delay.lock().await = Some(delay);
                    log::trace!("sent heartbeat, delay: {delay:?}");
                }
            }
        });

        // Authenticate
        let key = generate_secret_key("room_monitor", 64)
            .expect("failed to generate key for room monitor");
        let this = Self {
            state,
            stream,
            ping_fail_count,
            ping_task_handle,
        };
        this.authenticate(&key).await.map(move |_| this)
    }

    pub async fn authenticate(&self, key: &[u8]) -> Result<()> {
        let stream = Arc::clone(&self.stream);
        self.state
            .authenticate_result
            .acquire(async move || {
                stream
                    .send(ClientCommand::RoomMonitorAuthenticate { key: key.into() })
                    .await
            })
            .await?
            .map(|_| {})
            .map_err(Error::msg)
    }

    pub async fn ping(&self) -> Result<Duration> {
        let start = Instant::now();
        self.stream.send(ClientCommand::Ping).await?;
        time::timeout(HEARTBEAT_TIMEOUT, self.state.ping_notify.notified())
            .await
            .with_context(|| "heartbeat timeout")?;
        let delay = start.elapsed();
        *self.state.delay.lock().await = Some(delay);
        Ok(delay)
    }

    pub fn delay(&self) -> Option<Duration> {
        *self.state.delay.blocking_lock()
    }

    pub fn ping_fail_count(&self) -> u8 {
        self.ping_fail_count.load(Ordering::Relaxed)
    }

    pub async fn listen_stream(&self) -> impl futures::Stream<Item = Result<Event, Infallible>> {
        let room_state = self.state.cached_room_state.read().await;
        let events = self.state.cached_events.read().await;
        let mut init_events: Vec<Result<Event, Infallible>> = Vec::new();

        for (id, data) in &room_state.0 {
            let s = json!({"room": id.to_string(), "data": data.clone()}).to_string();
            init_events.push(Ok(Event::default().event("create_room").data(s)));
        }
        for event in events.iter() {
            init_events.push(Ok(event.clone()));
        }
        let init_stream = futures::stream::iter(init_events);
        let update_stream = BroadcastStream::new(self.state.broadcast_tx.subscribe())
            .map(|msg| msg.or_else(|_| Ok(Event::default().event("error").comment("lagged"))));
        init_stream.chain(update_stream)
    }

    async fn update_room_info(&self) -> Result<()> {
        let mut next_sync_time = self.state.next_sync_time.lock().await;
        if *next_sync_time < Instant::now() {
            let stream = Arc::clone(&self.stream);
            *self.state.cached_room_state.write().await = self
                .state
                .room_result
                .acquire(async move || stream.send(ClientCommand::QueryRoomInfo).await)
                .await?
                .map_err(Error::msg)?;
            self.state.cached_events.write().await.clear();
            *next_sync_time = Instant::now() + Duration::from_secs(1);
        }
        Ok(())
    }

    pub async fn get_room_list(&self) -> Result<Value> {
        self.update_room_info().await?;
        let mut res = Vec::new();
        for (id, data) in &self.state.cached_room_state.read().await.0 {
            res.push(json!({"name": id.to_string(), "data": data.clone()}));
        }
        Ok(res.into())
    }

    pub async fn get_room_by_id(&self, id: RoomId) -> Result<Value> {
        self.update_room_info().await?;
        let guard = self.state.cached_room_state.read().await;
        Ok(guard.0.get(&id).cloned().unwrap_or(Value::Null))
    }

    pub async fn get_room_of_user(&self, id: i32) -> Result<Value> {
        self.update_room_info().await?;
        let guard = self.state.cached_room_state.read().await;
        let id = match guard.1.get(&id).cloned() {
            Some(id) => id,
            None => return Ok(Value::Null),
        };
        Ok(guard.0.get(&id).cloned().unwrap_or(Value::Null))
    }
}

impl Drop for RoomMonitorClient {
    fn drop(&mut self) {
        self.ping_task_handle.abort();
    }
}

async fn process(state: Arc<ClientState>, cmd: ServerCommand) {
    match cmd {
        ServerCommand::Pong => {
            state.ping_notify.notify_one();
        }
        ServerCommand::Authenticate(res) => {
            let _ = state
                .authenticate_result
                .put(res)
                .await
                .inspect_err(|e| log::warn!("error setting authenticate result: {e}"));
        }
        ServerCommand::RoomResponse(value) => {
            let _ = state
                .room_result
                .put(value)
                .await
                .inspect_err(|e| log::warn!("error setting room result: {e}"));
        }
        ServerCommand::RoomEvent { event_type, data } => {
            let _ = state
                .push_event(Event::default().event(&event_type).data(data.to_string()))
                .await
                .inspect_err(|e| log::warn!("error sending {event_type} event: {e}"));
        }
        _ => {
            log::warn!("unsupported command: {cmd:?}, ignoring");
        }
    }
}
