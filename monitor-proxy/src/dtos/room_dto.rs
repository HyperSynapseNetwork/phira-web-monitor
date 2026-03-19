use phira_mp_common::RoomData;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RoomInfoResponse {
    pub name: String,
    pub data: RoomData,
}

#[derive(Debug, Serialize)]
pub struct RoomListResponse {
    pub rooms: Vec<RoomInfoResponse>,
    pub total: usize,
}
