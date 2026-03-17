use crate::{
    dtos::{RoomInfoResponse, RoomListResponse, VisitedUserInfo, VisitedUserListResponse},
    entity::visited_user,
    error::{AppErrorExt, Result},
    utils::{MpClient, MpClientState, SResult, TaskResult},
    AppState,
};
use anyhow::Error;
use axum::response::sse::Event;
use futures::StreamExt;
use log::warn;
use phira_mp_common::{
    generate_secret_key, ClientCommand, ClientRoomState, RoomData, RoomEvent, RoomId,
    ServerCommand, UserInfo,
};
use sea_orm::{sea_query::OnConflict, EntityTrait, PaginatorTrait};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_stream::wrappers::BroadcastStream;

type RoomMap = HashMap<RoomId, RoomData>;
type UserMap = HashMap<i32, RoomId>;

struct RoomMonitorState {
    authenticate_result: TaskResult<SResult<(UserInfo, Option<ClientRoomState>)>>,
    room_result: TaskResult<SResult<(RoomMap, UserMap)>>,

    /// (room state, update events, next sync time)
    cached_room_state: RwLock<(RoomMap, UserMap)>,
    cached_events: RwLock<Vec<Event>>,
    cached_visited_user: RwLock<HashSet<i32>>,
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
            ServerCommand::RoomEvent(event) => {
                match &event {
                    RoomEvent::CreateRoom { data, .. } if data.host != -1 => {
                        self.cached_visited_user.write().await.insert(data.host);
                    }
                    RoomEvent::JoinRoom { user, .. } => {
                        self.cached_visited_user.write().await.insert(*user);
                    }
                    _ => {}
                }
                let event_type = event.event_type();
                let data_str = event.inner().to_string();
                let _ = self
                    .push_event(Event::default().event(event_type).data(data_str))
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
            cached_visited_user: RwLock::default(),
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
            .acquire(|| {
                self.client
                    .send(ClientCommand::RoomMonitorAuthenticate { key: key.into() })
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
            init_events.push(Ok(Event::default().event("update_room").data(s)));
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
                .acquire(|| self.client.send(ClientCommand::QueryRoomInfo))
                .await?
                .map_err(Error::msg)
                .internal_server_error("failed to sync room info")?;
            self.client.cached_events.write().await.clear();
            *next_sync_time = Instant::now() + Duration::from_secs(1);
        }
        Ok(())
    }

    pub async fn get_visited(
        &self,
        state: &AppState,
        count_only: bool,
    ) -> Result<VisitedUserListResponse> {
        self.try_update_visited(state).await?;
        if count_only {
            Ok(VisitedUserListResponse {
                count: visited_user::Entity::find().count(&state.db).await?,
                users: None,
            })
        } else {
            let users: Vec<_> = visited_user::Entity::find()
                .all(&state.db)
                .await
                .internal_server_error("failed to get visited users")?
                .into_iter()
                .map(|m| VisitedUserInfo {
                    phira_id: m.phira_id,
                })
                .collect();
            Ok(VisitedUserListResponse {
                count: users.len() as u64,
                users: Some(users),
            })
        }
    }

    async fn try_update_visited(&self, state: &AppState) -> Result<()> {
        let cached_visited = std::mem::take(&mut *self.client.cached_visited_user.write().await);
        if cached_visited.is_empty() {
            return Ok(());
        }

        let iter = cached_visited.iter().map(|id| visited_user::ActiveModel {
            phira_id: sea_orm::Set(*id),
        });
        let on_conflict = OnConflict::column(visited_user::Column::PhiraId)
            .do_nothing()
            .to_owned();
        let res = visited_user::Entity::insert_many(iter)
            .on_conflict(on_conflict)
            .exec(&state.db)
            .await
            .internal_server_error("failed to update visited users");
        if res.is_err() {
            // put back data
            self.client
                .cached_visited_user
                .write()
                .await
                .extend(cached_visited);
        }
        res.map(|_| ())
    }
}
