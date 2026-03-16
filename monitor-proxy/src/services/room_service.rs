use crate::{
    dtos::{RoomInfoResponse, RoomListResponse},
    error::{AppErrorExt, Result},
    utils::{MpClient, MpClientState, SResult, TaskResult},
};
use anyhow::Error;
use axum::response::sse::Event;
use futures::StreamExt;
use log::warn;
use phira_mp_common::{
    generate_secret_key, ClientCommand, ClientRoomState, RoomId, ServerCommand, UserInfo,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    convert::Infallible,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_stream::wrappers::BroadcastStream;

struct RoomMonitorState {
    authenticate_result: TaskResult<SResult<(UserInfo, Option<ClientRoomState>)>>,
    room_result: TaskResult<SResult<(HashMap<RoomId, Value>, HashMap<i32, RoomId>)>>,

    /// (room state, update events, next sync time)
    cached_room_state: RwLock<(HashMap<RoomId, Value>, HashMap<i32, RoomId>)>,
    cached_events: RwLock<Vec<Event>>,
    next_sync_time: Mutex<Instant>,
    broadcast_tx: broadcast::Sender<Event>,
}

impl MpClientState for RoomMonitorState {
    async fn process(&self, cmd: ServerCommand) {
        match cmd {
            ServerCommand::Authenticate(res) => {
                let _ = self
                    .authenticate_result
                    .put(res)
                    .await
                    .inspect_err(|e| warn!("error setting authenticate result: {e}"));
            }
            ServerCommand::RoomResponse(value) => {
                let _ = self
                    .room_result
                    .put(value)
                    .await
                    .inspect_err(|e| warn!("error setting room result: {e}"));
            }
            ServerCommand::RoomEvent { event_type, data } => {
                let _ = self
                    .push_event(Event::default().event(&event_type).data(data.to_string()))
                    .await
                    .inspect_err(|e| warn!("error sending {event_type} event: {e}"));
            }
            _ => {
                warn!("unsupported command: {cmd:?}, ignoring");
            }
        }
    }
}

impl RoomMonitorState {
    pub fn new() -> Self {
        Self {
            authenticate_result: TaskResult::new(),
            room_result: TaskResult::new(),
            cached_room_state: RwLock::default(),
            cached_events: RwLock::default(),
            next_sync_time: Mutex::new(Instant::now()),
            broadcast_tx: broadcast::channel(1024).0,
        }
    }

    pub async fn push_event(&self, event: Event) -> anyhow::Result<()> {
        let mut events = self.cached_events.write().await;
        events.push(event.clone());
        self.broadcast_tx.send(event)?;
        Ok(())
    }
}

pub struct RoomService {
    client: MpClient<RoomMonitorState>,
}

impl RoomService {
    pub async fn new(mp_server: &str) -> anyhow::Result<Self> {
        let client = MpClient::new(mp_server, RoomMonitorState::new()).await?;
        let key = generate_secret_key("room_monitor", 64)
            .expect("failed to generate key for room monitor");
        let this = Self { client };
        this.authenticate(&key).await?;
        Ok(this)
    }

    pub async fn authenticate(&self, key: &[u8]) -> anyhow::Result<()> {
        self.client
            .authenticate_result
            .acquire(async move || {
                self.client
                    .send(ClientCommand::RoomMonitorAuthenticate { key: key.into() })
                    .await
            })
            .await?
            .map(|_| {})
            .map_err(Error::msg)
    }

    pub async fn listen_stream(&self) -> impl futures::Stream<Item = Result<Event, Infallible>> {
        let room_state = self.client.cached_room_state.read().await;
        let events = self.client.cached_events.read().await;
        let mut init_events = Vec::new();
        for (id, data) in &room_state.0 {
            let s = json!({"room": id.to_string(), "data": data.clone()}).to_string();
            init_events.push(Ok(Event::default().event("create_room").data(s)));
        }
        for event in events.iter() {
            init_events.push(Ok(event.clone()));
        }
        let init_stream = futures::stream::iter(init_events);
        let update_stream = BroadcastStream::new(self.client.broadcast_tx.subscribe())
            .map(|msg| msg.or_else(|_| Ok(Event::default().event("error").comment("lagged"))));
        init_stream.chain(update_stream)
    }

    pub async fn get_room_list(&self) -> Result<RoomListResponse> {
        self.update_room_info().await?;
        let guard = self.client.cached_room_state.read().await;
        let rooms = guard
            .0
            .iter()
            .map(|(id, data)| RoomInfoResponse {
                name: id.to_string(),
                data: data.clone(),
            })
            .collect::<Vec<_>>();
        Ok(RoomListResponse {
            total: rooms.len(),
            rooms,
        })
    }

    pub async fn get_room_by_id(&self, id: RoomId) -> Result<RoomInfoResponse> {
        self.update_room_info().await?;
        let guard = self.client.cached_room_state.read().await;
        guard
            .0
            .get(&id)
            .not_found("no such room")
            .map(|data| RoomInfoResponse {
                name: id.to_string(),
                data: data.clone(),
            })
    }

    pub async fn get_room_of_user(&self, id: i32) -> Result<Option<RoomInfoResponse>> {
        self.update_room_info().await?;
        let guard = self.client.cached_room_state.read().await;
        let res = guard
            .1
            .get(&id)
            .and_then(|room_id| Some((room_id, guard.0.get(room_id)?)))
            .map(|(room_id, data)| RoomInfoResponse {
                name: room_id.to_string(),
                data: data.clone(),
            });
        Ok(res)
    }

    async fn update_room_info(&self) -> Result<()> {
        let mut next_sync_time = self.client.next_sync_time.lock().await;
        if *next_sync_time < Instant::now() {
            *self.client.cached_room_state.write().await = self
                .client
                .room_result
                .acquire(async move || self.client.send(ClientCommand::QueryRoomInfo).await)
                .await?
                .map_err(Error::msg)
                .internal_server_error("failed to sync room info")?;
            self.client.cached_events.write().await.clear();
            *next_sync_time = Instant::now() + Duration::from_secs(1);
        }
        Ok(())
    }
}
