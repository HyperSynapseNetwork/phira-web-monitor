use super::LiveEvent;
use crate::utils::TaskResult;
use anyhow::{anyhow, Context, Result};
use log::{info, warn};
use phira_mp_common::*;
use std::sync::{
    atomic::{AtomicI32, AtomicU8, Ordering},
    Arc,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, Mutex, Notify},
    task::JoinHandle,
    time::{self, Duration, Instant},
};

pub struct GameMonitorClient {
    state: Arc<GameMonitorState>,
    stream: Arc<Stream<ClientCommand, ServerCommand>>,

    _ping_task_handle: JoinHandle<()>,
}

struct GameMonitorState {
    delay: Mutex<Option<Duration>>,
    ping_notify: Notify,

    auth_result: TaskResult<SResult<(UserInfo, Option<ClientRoomState>)>>,
    join_result: TaskResult<SResult<JoinRoomResponse>>,
    leave_result: TaskResult<SResult<()>>,

    event_tx: mpsc::UnboundedSender<LiveEvent>,
    selected_chart: AtomicI32,
}

impl GameMonitorClient {
    pub async fn new(
        addr: &str,
        token: &str,
        event_tx: mpsc::UnboundedSender<LiveEvent>,
    ) -> Result<Self> {
        let tcp = TcpStream::connect(addr).await?;
        tcp.set_nodelay(true)?;

        let state = Arc::new(GameMonitorState {
            delay: Mutex::new(None),
            ping_notify: Notify::new(),
            auth_result: TaskResult::new(),
            join_result: TaskResult::new(),
            leave_result: TaskResult::new(),
            event_tx,
            selected_chart: AtomicI32::new(-1),
        });
        let stream = Arc::new(
            Stream::new(
                Some(1),
                tcp,
                Box::new({
                    let state = Arc::clone(&state);
                    move |_tx, cmd| {
                        let state = Arc::clone(&state);
                        async move {
                            process(state, cmd).await;
                        }
                    }
                }),
            )
            .await?,
        );

        // Authenticate with GameMonitorAuthenticate
        let auth_res = state
            .auth_result
            .acquire({
                let stream = Arc::clone(&stream);
                || async move {
                    let token: Varchar<32> = token
                        .to_owned()
                        .try_into()
                        .with_context(|| format!("failed to convert token `{token}`"))?;
                    stream
                        .send(ClientCommand::GameMonitorAuthenticate { token })
                        .await
                        .with_context(|| "failed to send authentication")?;
                    Ok(())
                }
            })
            .await?;

        let (user_info, room_state) = auth_res.map_err(|e| anyhow!(e))?;
        info!(
            "Game monitor authenticated as: {} ({})",
            user_info.name, user_info.id
        );

        // Forward auth success to WS client
        let _ = state.event_tx.send(LiveEvent::Authenticate(Ok((
            user_info.clone(),
            room_state.clone(),
        ))));

        let _ping_task_handle = tokio::spawn({
            let stream = Arc::clone(&stream);
            let state = Arc::clone(&state);
            async move {
                let ping_fail_count = AtomicU8::new(0);
                loop {
                    time::sleep(HEARTBEAT_INTERVAL).await;
                    let start = Instant::now();
                    if stream.send(ClientCommand::Ping).await.is_err() {
                        info!("Game monitor: failed to send heartbeat");
                        break;
                    }
                    match time::timeout(HEARTBEAT_TIMEOUT, state.ping_notify.notified()).await {
                        Ok(_) => {
                            *state.delay.lock().await = Some(start.elapsed());
                            ping_fail_count.store(0, Ordering::Relaxed);
                        }
                        Err(_) => {
                            let fails = ping_fail_count.fetch_add(1, Ordering::Relaxed) + 1;
                            if fails >= 3 {
                                info!("Game monitor: ping timeout, disconnecting");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            state,
            stream,
            _ping_task_handle,
        })
    }

    pub async fn join_room(&self, room_id: RoomId) -> Result<SResult<JoinRoomResponse>> {
        let res = self
            .state
            .join_result
            .acquire({
                let stream = Arc::clone(&self.stream);
                || async move {
                    stream
                        .send(ClientCommand::JoinRoom {
                            id: room_id,
                            monitor: true,
                        })
                        .await
                }
            })
            .await?;
        // Also forward as event
        let _ = self.state.event_tx.send(LiveEvent::Join(res.clone()));
        Ok(res)
    }

    pub async fn leave_room(&self) -> Result<SResult<()>> {
        let res = self
            .state
            .leave_result
            .acquire({
                let stream = Arc::clone(&self.stream);
                || async move { stream.send(ClientCommand::LeaveRoom).await }
            })
            .await?;
        let _ = self.state.event_tx.send(LiveEvent::Leave(res.clone()));
        Ok(res)
    }
}

async fn process(state: Arc<GameMonitorState>, cmd: ServerCommand) {
    let tx = &state.event_tx;
    match cmd {
        ServerCommand::Pong => state.ping_notify.notify_one(),
        ServerCommand::Authenticate(r) => {
            state.auth_result.put(r).await.ok();
        }
        ServerCommand::JoinRoom(r) => {
            state.join_result.put(r).await.ok();
        }
        ServerCommand::LeaveRoom(r) => {
            state.leave_result.put(r).await.ok();
        }
        ServerCommand::Touches { player, frames } => {
            let _ = tx.send(LiveEvent::Touches {
                player,
                frames: (*frames).clone(),
            });
        }
        ServerCommand::Judges { player, judges } => {
            let _ = tx.send(LiveEvent::Judges {
                player,
                judges: (*judges).clone(),
            });
        }
        ServerCommand::Message(msg) => {
            if let Message::SelectChart { id, .. } = &msg {
                state.selected_chart.store(*id, Ordering::Relaxed);
            }
            let _ = tx.send(LiveEvent::Message(msg));
        }
        ServerCommand::ChangeState(s) => {
            let _ = tx.send(LiveEvent::StateChange(s));
        }
        ServerCommand::OnJoinRoom(info) => {
            let _ = tx.send(LiveEvent::UserJoin(info));
        }
        _ => {
            warn!("Invalid command received: {cmd:?}");
        }
    }
}
