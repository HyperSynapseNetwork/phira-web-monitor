use crate::{
    error::{AppErrorExt, Result},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
    Json,
};
use futures::Stream;
use phira_mp_common::RoomId;
use serde::Deserialize;
use std::{convert::Infallible, time::Duration};

pub async fn get_room_list(State(state): State<AppState>) -> Result<Response> {
    state
        .room_service
        .get_room_list()
        .await
        .map(|s| Json(s).into_response())
}

pub async fn get_room_by_id(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response> {
    state
        .room_service
        .get_room_by_id(RoomId::try_from(id).bad_request("invalid room ID")?)
        .await
        .map(|s| Json(s).into_response())
}

pub async fn get_room_of_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Response> {
    state
        .room_service
        .get_room_of_user(id)
        .await
        .map(|s| Json(s).into_response())
}

pub async fn listen(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    Sse::new(state.room_service.listen_stream().await)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
}

#[derive(Deserialize)]
pub struct GetVisitedUsersParams {
    #[serde(default)]
    count_only: bool,
}

pub async fn get_visited_users(
    State(state): State<AppState>,
    Query(params): Query<GetVisitedUsersParams>,
) -> Result<Response> {
    state
        .room_service
        .get_visited(&state, params.count_only)
        .await
        .map(|s| Json(s).into_response())
}
