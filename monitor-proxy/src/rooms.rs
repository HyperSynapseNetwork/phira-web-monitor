use std::{convert::Infallible, time::Duration};

use crate::{json_err, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
    Json,
};
use serde_json::json;

use phira_mp_common::RoomId;

mod client;
pub use client::*;

pub async fn get_room_list(State(state): State<AppState>) -> (StatusCode, Response) {
    state
        .room_monitor_client
        .get_room_list()
        .await
        .map(|s| (StatusCode::OK, Json(s).into_response()))
        .unwrap_or_else(|e| (StatusCode::INTERNAL_SERVER_ERROR, json_err!("{e}")))
}

pub async fn get_room_by_id(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Response) {
    let id = match RoomId::try_from(id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, json_err!("invalid room id: {e}")),
    };
    state
        .room_monitor_client
        .get_room_by_id(id)
        .await
        .map(|s| (StatusCode::OK, Json(s).into_response()))
        .unwrap_or_else(|e| (StatusCode::INTERNAL_SERVER_ERROR, json_err!("{e}")))
}

pub async fn get_room_of_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> (StatusCode, Response) {
    state
        .room_monitor_client
        .get_room_of_user(id)
        .await
        .map(|s| (StatusCode::OK, Json(s).into_response()))
        .unwrap_or_else(|e| (StatusCode::INTERNAL_SERVER_ERROR, json_err!("{e}")))
}

pub async fn listen(
    State(state): State<AppState>,
) -> (
    StatusCode,
    Sse<impl futures::Stream<Item = Result<Event, Infallible>>>,
) {
    (
        StatusCode::OK,
        Sse::new(state.room_monitor_client.listen_stream().await)
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(10))),
    )
}
