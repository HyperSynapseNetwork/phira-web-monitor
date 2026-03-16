use serde::Serialize;
use serde_json::Value as JsonValue;

#[derive(Debug, Serialize)]
pub struct RoomInfoResponse {
    pub name: String,
    pub data: JsonValue,
}

#[derive(Debug, Serialize)]
pub struct RoomListResponse {
    pub rooms: Vec<RoomInfoResponse>,
    pub total: usize,
}
