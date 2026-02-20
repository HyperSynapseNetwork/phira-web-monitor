//! Live monitoring client — WebSocket connection and event dispatcher.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use monitor_common::live::{LiveEvent, WsCommand};
use phira_mp_common::{decode_packet, encode_packet};
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use crate::console_log;

/// Live monitoring client — manages a WebSocket connection to the proxy
/// and exposes an event queue for the JS-driven render loop.
#[wasm_bindgen]
pub struct Monitor {
    ws: WebSocket,
    #[wasm_bindgen(skip)]
    pub event_queue: Rc<RefCell<VecDeque<LiveEvent>>>,

    // prevent GC of closures
    #[wasm_bindgen(skip)]
    pub _onmessage: Closure<dyn FnMut(MessageEvent)>,
    #[wasm_bindgen(skip)]
    pub _onclose: Closure<dyn FnMut(CloseEvent)>,
    #[wasm_bindgen(skip)]
    pub _onerror: Closure<dyn FnMut(ErrorEvent)>,
}

#[wasm_bindgen]
impl Monitor {
    /// Create a new Monitor and connect to the live WebSocket endpoint.
    ///
    /// `ws_url` should be the full WebSocket URL, e.g. `wss://example.com/ws/live`
    #[wasm_bindgen(constructor)]
    pub fn new(ws_url: &str) -> Result<Monitor, JsValue> {
        console_error_panic_hook::set_once();

        let ws = WebSocket::new(ws_url)?;
        ws.set_binary_type(BinaryType::Arraybuffer);

        let event_queue: Rc<RefCell<VecDeque<LiveEvent>>> = Rc::new(RefCell::new(VecDeque::new()));

        // onmessage: decode binary frames into LiveEvent and push to queue
        let onmessage = {
            let queue = Rc::clone(&event_queue);
            Closure::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                    let arr = js_sys::Uint8Array::new(&buf);
                    let data = arr.to_vec();
                    match decode_packet::<LiveEvent>(&data) {
                        Ok(event) => {
                            queue.borrow_mut().push_back(event);
                        }
                        Err(err) => {
                            console_log!("Monitor: failed to decode LiveEvent: {:?}", err);
                        }
                    }
                }
            }) as Box<dyn FnMut(MessageEvent)>)
        };
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        // onclose
        let onclose = Closure::wrap(Box::new(move |e: CloseEvent| {
            console_log!(
                "Monitor WS closed: code={}, reason={}",
                e.code(),
                e.reason()
            );
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));

        // onerror
        let onerror = Closure::wrap(Box::new(move |_e: ErrorEvent| {
            console_log!("Monitor WS error");
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        console_log!("Monitor: connecting to {}", ws_url);

        Ok(Monitor {
            ws,
            event_queue,
            _onmessage: onmessage,
            _onclose: onclose,
            _onerror: onerror,
        })
    }

    /// Send a JoinRoom command over the WebSocket.
    pub fn join_room(&self, room_id: &str) -> Result<(), JsValue> {
        let id = room_id
            .to_string()
            .try_into()
            .map_err(|e| JsValue::from_str(&format!("Invalid room ID: {:?}", e)))?;
        let cmd = WsCommand::Join { room_id: id };
        self.send_command(&cmd)
    }

    /// Send a LeaveRoom command over the WebSocket.
    pub fn leave_room(&self) -> Result<(), JsValue> {
        self.send_command(&WsCommand::Leave)
    }

    /// Drain all pending events and log them (Stage 2 stub).
    /// In Stage 3 this will dispatch to GameScene instances.
    pub fn tick(&mut self, _timestamp: f64) -> Result<(), JsValue> {
        let events: Vec<LiveEvent> = self.event_queue.borrow_mut().drain(..).collect();
        for event in &events {
            match event {
                LiveEvent::Authenticate(Ok((info, _))) => {
                    console_log!("Monitor: authenticated as {} (id={})", info.name, info.id);
                }
                LiveEvent::Authenticate(Err(e)) => {
                    console_log!("Monitor: auth failed: {}", e);
                }
                LiveEvent::Join(Ok(resp)) => {
                    console_log!("Monitor: joined room, {} users", resp.users.len());
                }
                LiveEvent::Join(Err(e)) => {
                    console_log!("Monitor: join failed: {}", e);
                }
                LiveEvent::Leave(r) => {
                    console_log!("Monitor: leave result: {:?}", r);
                }
                LiveEvent::StateChange(state) => {
                    console_log!("Monitor: state change: {:?}", state);
                }
                LiveEvent::UserJoin(info) => {
                    console_log!("Monitor: user joined: {} (id={})", info.name, info.id);
                }
                LiveEvent::UserLeave { user } => {
                    console_log!("Monitor: user left: id={}", user);
                }
                LiveEvent::Touches { player, frames } => {
                    console_log!(
                        "Monitor: touches from player {}, {} frames",
                        player,
                        frames.len()
                    );
                }
                LiveEvent::Judges { player, judges } => {
                    console_log!(
                        "Monitor: judges from player {}, {} events",
                        player,
                        judges.len()
                    );
                }
                LiveEvent::Message(msg) => {
                    console_log!("Monitor: message: {:?}", msg);
                }
            }
        }
        Ok(())
    }

    /// Close the WebSocket connection.
    pub fn close(&self) -> Result<(), JsValue> {
        self.ws.close()
    }
}

impl Monitor {
    fn send_command(&self, cmd: &WsCommand) -> Result<(), JsValue> {
        let mut buf = Vec::new();
        encode_packet(cmd, &mut buf);
        self.ws.send_with_u8_array(&buf)
    }
}
