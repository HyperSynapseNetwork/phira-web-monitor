use crate::{json_err, AppState};
use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

use tokio::{net::TcpStream, sync::oneshot};

use phira_mp_common::{ClientCommand, RoomId, ServerCommand, Stream};

pub async fn query_rooms(State(state): State<AppState>) -> (StatusCode, Response) {
    query_rooms_inner(&state.args.mp_server, None)
        .await
        .map(|s| (StatusCode::OK, Json(s).into_response()))
        .unwrap_or_else(|e| {
            log::error!("Error querying rooms: {e:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, json_err!("{e}"))
        })
}

pub async fn query_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Response) {
    query_rooms_inner(&state.args.mp_server, Some(id))
        .await
        .map(|s| (StatusCode::OK, Json(s).into_response()))
        .unwrap_or_else(|e| {
            log::error!("Error querying rooms: {e:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, json_err!("{e}"))
        })
}

async fn query_rooms_inner(mp_server: &str, id: Option<String>) -> Result<Value> {
    let (tx, rx) = oneshot::channel::<String>();
    let stream = Stream::<ClientCommand, ServerCommand>::new(
        Some(1),
        TcpStream::connect(mp_server).await?,
        Box::new({
            let mut tx = Some(tx);
            move |_, cmd| {
                let tx = tx.take();
                async move {
                    let Some(tx) = tx else { return };
                    if let ServerCommand::ResponseRooms(rooms_data) = cmd {
                        let _ = tx.send(rooms_data);
                    } else {
                        log::warn!("Unknown command received: {cmd:?}");
                    }
                }
            }
        }),
    )
    .await?;
    stream
        .send(ClientCommand::QueryRooms {
            id: match id {
                Some(id) => Some(RoomId::try_from(id)?),
                None => None,
            },
        })
        .await?;
    serde_json::from_str(&rx.await?).with_context(|| "invalid json value")
}
