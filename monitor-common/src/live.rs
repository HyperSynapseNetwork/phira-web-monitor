//! Live monitoring protocol types shared between proxy and client.

#[allow(unused_imports)]
use crate::Result; // needed by BinaryData derive macro (generates bare `Result<T>`)
use phira_mp_common::*;
use phira_mp_macros::BinaryData;

// Re-export for phira_mp_macros derive (generates `crate::BinaryData` etc.)
// These must be visible at the crate root — we re-export from lib.rs.

/// Commands sent from the browser to the proxy over WebSocket
#[derive(Debug, BinaryData)]
#[repr(u8)]
pub enum WsCommand {
    Join { room_id: RoomId },
    Leave,
    Ready,
}

/// Events sent from the proxy to the browser over WebSocket
#[derive(Clone, Debug, BinaryData)]
#[repr(u8)]
pub enum LiveEvent {
    Authenticate(SResult<(UserInfo, Option<ClientRoomState>)>),
    Join(SResult<JoinRoomResponse>),
    Leave(SResult<()>),

    Touches {
        player: i32,
        frames: Vec<TouchFrame>,
    },
    Judges {
        player: i32,
        judges: Vec<JudgeEvent>,
    },
    StateChange(RoomState),
    UserJoin(UserInfo),
    UserLeave {
        user: i32,
    },
    Message(Message),
}
