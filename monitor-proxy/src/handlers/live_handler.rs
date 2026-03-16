use crate::{middlewares::AuthSession, AppState};
use axum::{
    extract::{ws::Message as WsMessage, Extension, State, WebSocketUpgrade},
    response::Response,
};
use futures::{SinkExt, StreamExt};
use log::{error, info};
use monitor_common::live::WsCommand;
use phira_mp_common::{decode_packet, encode_packet};

pub async fn live_ws(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    ws: WebSocketUpgrade,
) -> Response {
    info!("Live WS upgrade request from user {}", session.id);
    ws.on_upgrade(async move |socket| {
        let (mut event_rx, client) = match state.live_service.connect(&state, &session).await {
            Ok(res) => res,
            Err(e) => {
                error!("Failed to create GameMonitorClient: {e:?}");
                return;
            }
        };
        info!("Live WS connected for user {}", session.id);

        let (mut ws_tx, mut ws_rx) = socket.split();
        let task1 = async {
            while let Some(event) = event_rx.recv().await {
                let mut buf = Vec::new();
                encode_packet(&event, &mut buf);
                if ws_tx.send(WsMessage::Binary(buf.into())).await.is_err() {
                    break;
                }
            }
        };
        let task2 = async {
            while let Some(Ok(msg)) = ws_rx.next().await {
                match msg {
                    WsMessage::Binary(data) => match decode_packet::<WsCommand>(&data) {
                        Ok(WsCommand::Join { room_id }) => {
                            if let Err(e) = client.join_room(room_id).await {
                                error!("Join room failed: {e:?}");
                            }
                        }
                        Ok(WsCommand::Leave) => {
                            if let Err(e) = client.leave_room().await {
                                error!("Leave room failed: {e:?}");
                            }
                        }
                        Ok(WsCommand::Ready) => {
                            if let Err(e) = client.send_ready().await {
                                error!("Send ready failed: {e:?}");
                            }
                        }
                        Err(e) => error!("Failed to decode WS command: {e:?}"),
                    },
                    WsMessage::Close(_) => break,
                    _ => {}
                }
            }
        };
        tokio::select! { _ = task1 => {}, _ = task2 => {} };
    })
}
