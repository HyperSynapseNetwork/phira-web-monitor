use crate::{
    error::Result,
    middlewares::AuthSession,
    utils::{MpClient, MpClientState, SResult, TaskResult},
    AppState,
};
use anyhow::{Context, Error};
use log::{error, info, warn};
use monitor_common::live::LiveEvent;
use phira_mp_common::{
    ClientCommand, ClientRoomState, JoinRoomResponse, Message, RoomId, ServerCommand, UserInfo,
    Varchar,
};
use std::{
    ops::Deref,
    sync::atomic::{AtomicI32, Ordering},
};
use tokio::sync::mpsc;

pub struct GameMonitorState {
    auth_result: TaskResult<SResult<(UserInfo, Option<ClientRoomState>)>>,
    join_result: TaskResult<SResult<JoinRoomResponse>>,
    leave_result: TaskResult<SResult<()>>,

    event_tx: mpsc::UnboundedSender<LiveEvent>,
    selected_chart: AtomicI32,
}

impl GameMonitorState {
    pub fn new(event_tx: mpsc::UnboundedSender<LiveEvent>) -> Self {
        GameMonitorState {
            auth_result: TaskResult::new(),
            join_result: TaskResult::new(),
            leave_result: TaskResult::new(),

            event_tx,
            selected_chart: AtomicI32::new(-1),
        }
    }
}

impl MpClientState for GameMonitorState {
    async fn process(&self, cmd: ServerCommand) {
        let tx = &self.event_tx;
        match cmd {
            ServerCommand::Authenticate(r) => {
                self.auth_result.put(r).await.ok();
            }
            ServerCommand::JoinRoom(r) => {
                self.join_result.put(r).await.ok();
            }
            ServerCommand::LeaveRoom(r) => {
                self.leave_result.put(r).await.ok();
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
                    self.selected_chart.store(*id, Ordering::Relaxed);
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
}

pub struct GameMonitorClient(MpClient<GameMonitorState>);

impl Deref for GameMonitorClient {
    type Target = MpClient<GameMonitorState>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GameMonitorClient {
    pub async fn new(
        mp_server: &str,
        token: &str,
        event_tx: mpsc::UnboundedSender<LiveEvent>,
    ) -> Result<Self> {
        let this = Self(MpClient::new(mp_server, GameMonitorState::new(event_tx)).await?);

        let auth_res = this
            .authenticate(token)
            .await
            .inspect_err(|e| error!("Failed to authenticate game monitor: {e}"))?;

        info!("Game monitor authenticated: {:?}", auth_res.0);
        let _ = this
            .event_tx
            .send(LiveEvent::Authenticate(Ok(auth_res)))
            .context("failed to send event");
        Ok(this)
    }

    pub async fn authenticate(
        &self,
        token: &str,
    ) -> anyhow::Result<(UserInfo, Option<ClientRoomState>)> {
        self.auth_result
            .acquire({
                || async {
                    let token: Varchar<32> = token
                        .to_owned()
                        .try_into()
                        .with_context(|| format!("failed to convert token `{token}`"))?;
                    self.send(ClientCommand::GameMonitorAuthenticate { token })
                        .await
                        .context("failed to send authentication")?;
                    Ok(())
                }
            })
            .await?
            .map_err(Error::msg)
    }

    pub async fn join_room(&self, room_id: RoomId) -> anyhow::Result<SResult<JoinRoomResponse>> {
        let res = self
            .join_result
            .acquire({
                || async move {
                    self.send(ClientCommand::JoinRoom {
                        id: room_id,
                        monitor: true,
                    })
                    .await
                }
            })
            .await?;
        // Also forward as event
        let _ = self.event_tx.send(LiveEvent::Join(res.clone()));
        Ok(res)
    }

    pub async fn leave_room(&self) -> anyhow::Result<SResult<()>> {
        let res = self
            .leave_result
            .acquire(|| self.send(ClientCommand::LeaveRoom))
            .await?;
        let _ = self.event_tx.send(LiveEvent::Leave(res.clone()));
        Ok(res)
    }

    pub async fn send_ready(&self) -> anyhow::Result<()> {
        self.send(ClientCommand::Ready).await?;
        Ok(())
    }
}

impl Drop for GameMonitorClient {
    fn drop(&mut self) {
        let _ = self.blocking_send(ClientCommand::LeaveRoom);
    }
}

pub struct LiveService {}

impl LiveService {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn connect(
        &self,
        state: &AppState,
        session: &AuthSession,
    ) -> Result<(mpsc::UnboundedReceiver<LiveEvent>, GameMonitorClient)> {
        let mp_server = state.config.mp_server.as_str();
        let token = session.token.as_str();
        let (tx, rx) = mpsc::unbounded_channel();
        Ok((rx, GameMonitorClient::new(mp_server, token, tx).await?))
    }
}

impl Default for LiveService {
    fn default() -> Self {
        Self::new()
    }
}
