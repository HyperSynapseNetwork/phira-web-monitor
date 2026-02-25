use crate::{auth::AuthSession, AppState};
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    Extension,
};
use futures::{stream::StreamExt, SinkExt};
use log::{error, info};
pub use monitor_common::live::{LiveEvent, WsCommand};
use phira_mp_common::*;
use tokio::sync::mpsc;

mod client;
pub use client::*;

pub async fn live_ws(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    info!("Live WS upgrade request from user {}", session.id);
    ws.on_upgrade(move |socket| handle_ws(state, session, socket))
}

async fn handle_ws(state: AppState, session: AuthSession, socket: WebSocket) {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    // Connect to phira-mp server and authenticate
    let client = match GameMonitorClient::new(&state.args.mp_server, &session.token, event_tx).await
    {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create GameMonitorClient: {e:?}");
            // Can't send error back easily since we need to split first
            // Just log and return
            return;
        }
    };

    info!("Live WS connected for user {}", session.id);

    // Split the socket for bidirectional forwarding
    let (mut ws_tx, mut ws_rx) = socket.split();

    tokio::select! {
        // Proxy → Browser: drain events and send as binary WS frames
        _ = async {
            while let Some(event) = event_rx.recv().await {
                let mut buf = Vec::new();
                encode_packet(&event, &mut buf);
                if ws_tx.send(WsMessage::Binary(buf.into())).await.is_err() {
                    break;
                }
            }
        } => {},
        // Browser → Proxy: receive WS messages and execute commands
        _ = async {
            while let Some(Ok(msg)) = ws_rx.next().await {
                match msg {
                    WsMessage::Binary(data) => {
                        if let Ok(cmd) = decode_packet::<WsCommand>(&data) {
                            match cmd {
                                WsCommand::Join { room_id } => {
                                    if let Err(e) = client.join_room(room_id).await {
                                        error!("Join room failed: {e:?}");
                                    }
                                }
                                WsCommand::Leave => {
                                    if let Err(e) = client.leave_room().await {
                                        error!("Leave room failed: {e:?}");
                                    }
                                }
                                WsCommand::Ready => {
                                    if let Err(e) = client.send_ready().await {
                                        error!("Send ready failed: {e:?}");
                                    }
                                }
                            }
                        }
                    }
                    WsMessage::Close(_) => break,
                    _ => {}
                }
            }
        } => {},
    }

    info!("Live WS disconnected for user {}", session.id);
    let _ = client.leave_room().await;
    drop(client);
}
