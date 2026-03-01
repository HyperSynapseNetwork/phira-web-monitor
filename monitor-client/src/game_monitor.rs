//! Live monitoring client — WebSocket connection and event dispatcher.

mod game_scene;
pub use game_scene::GameScene;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use monitor_common::core::{Chart, ChartInfo};
use monitor_common::live::{LiveEvent, WsCommand};
use phira_mp_common::{Message, RoomState, decode_packet, encode_packet};
use wasm_bindgen_futures::spawn_local;

use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use crate::console_log;

/// Live monitoring client — manages a WebSocket connection to the proxy,
/// dispatches events to per-player `GameScene` instances, and drives
/// the render loop.
#[wasm_bindgen]
pub struct GameMonitor {
    ws: WebSocket,
    #[wasm_bindgen(skip)]
    pub event_queue: Rc<RefCell<VecDeque<LiveEvent>>>,

    // Per-player rendering contexts
    #[wasm_bindgen(skip)]
    pub scenes: HashMap<i32, GameScene>,

    /// Currently selected chart ID (from SelectChart message)
    selected_chart_id: Option<i32>,
    /// API base URL for chart fetching
    api_base: String,

    #[wasm_bindgen(skip)]
    pub chart_info: Option<ChartInfo>,
    #[wasm_bindgen(skip)]
    pub chart_data: Option<Chart>,

    // Internal queue for asynchronously downloaded charts
    #[wasm_bindgen(skip)]
    pub pending_chart: Rc<RefCell<Option<(ChartInfo, Chart)>>>,

    // Prevent GC of closures
    #[wasm_bindgen(skip)]
    pub _onmessage: Closure<dyn FnMut(MessageEvent)>,
    #[wasm_bindgen(skip)]
    pub _onclose: Closure<dyn FnMut(CloseEvent)>,
    #[wasm_bindgen(skip)]
    pub _onerror: Closure<dyn FnMut(ErrorEvent)>,
}

#[wasm_bindgen]
impl GameMonitor {
    /// Create a new GameMonitor and connect to the live WebSocket endpoint.
    ///
    /// `ws_url` should be the full WebSocket URL, e.g. `wss://example.com/ws/live`
    /// `api_base` is the base URL for REST API calls (chart fetching)
    #[wasm_bindgen(constructor)]
    pub fn new(ws_url: &str, api_base: &str) -> Result<GameMonitor, JsValue> {
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
                            console_log!("GameMonitor: failed to decode LiveEvent: {:?}", err);
                        }
                    }
                }
            }) as Box<dyn FnMut(MessageEvent)>)
        };
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        // onclose
        let onclose = Closure::wrap(Box::new(move |e: CloseEvent| {
            console_log!(
                "GameMonitor WS closed: code={}, reason={}",
                e.code(),
                e.reason()
            );
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));

        // onerror
        let onerror = Closure::wrap(Box::new(move |_e: ErrorEvent| {
            console_log!("GameMonitor WS error");
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        console_log!("GameMonitor: connecting to {}", ws_url);

        Ok(GameMonitor {
            ws,
            event_queue,
            scenes: HashMap::new(),
            selected_chart_id: None,
            api_base: api_base.to_string(),
            chart_info: None,
            chart_data: None,
            pending_chart: Rc::new(RefCell::new(None)),
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

    /// Attach a `<canvas>` element to an existing headless scene.
    /// If no scene exists for this user, creates a headless one first.
    pub fn attach_canvas(&mut self, user_id: i32, canvas_id: &str) -> Result<(), JsValue> {
        // Ensure a headless scene exists
        self.scenes
            .entry(user_id)
            .or_insert_with(|| GameScene::new_headless(user_id));

        let scene = self.scenes.get_mut(&user_id).unwrap();

        // Immediately load chart if we already have it and scene doesn't have one
        if let (Some(info), Some(data)) = (&self.chart_info, &self.chart_data) {
            if !scene.has_chart() {
                scene.load_chart(info.clone(), data.clone());
            }
        }

        scene.attach_canvas(canvas_id)?;
        console_log!("GameMonitor: attached canvas for user {}", user_id);
        Ok(())
    }

    /// Detach the canvas from a scene (frees WebGL + Audio, keeps headless state).
    pub fn detach_canvas(&mut self, user_id: i32) {
        if let Some(scene) = self.scenes.get_mut(&user_id) {
            scene.detach_canvas();
            console_log!("GameMonitor: detached canvas for user {}", user_id);
        }
    }

    /// Resize a specific scene's canvas.
    pub fn resize_scene(&mut self, user_id: i32, width: u32, height: u32) {
        if let Some(scene) = self.scenes.get_mut(&user_id) {
            scene.resize(width, height);
        }
    }

    /// Fully remove the GameScene for the given user (e.g. user left the room).
    pub fn destroy_scene(&mut self, user_id: i32) {
        if self.scenes.remove(&user_id).is_some() {
            console_log!("GameMonitor: destroyed scene for user {}", user_id);
        }
    }

    /// Start playback for all scenes (e.g. when room transitions to Playing).
    pub fn start_all_scenes(&mut self) {
        for scene in self.scenes.values_mut() {
            scene.start();
        }
    }

    /// Load chart data provided by the frontend (fetched via API)
    /// and apply it to all currently active scenes.
    fn load_chart(&mut self, info: ChartInfo, chart: Chart) {
        self.chart_info = Some(info.clone());
        self.chart_data = Some(chart.clone());

        for (uid, scene) in self.scenes.iter_mut() {
            scene.load_chart(info.clone(), chart.clone());
            console_log!("GameMonitor: applied chart to scene for user {}", uid);
        }
    }

    /// Get the currently selected chart ID, if any.
    pub fn get_selected_chart_id(&self) -> Option<i32> {
        self.selected_chart_id
    }

    /// Get the API base URL.
    pub fn get_api_base(&self) -> String {
        self.api_base.clone()
    }

    /// Drain all pending events, dispatch to scenes, and render.
    ///
    /// `timestamp` is `performance.now()` in milliseconds (from rAF).
    pub fn tick(&mut self, timestamp: f64) -> Result<(), JsValue> {
        let pending = self.pending_chart.borrow_mut().take();
        if let Some((info, chart)) = pending {
            console_log!("GameMonitor: processing internally fetched chart...");
            self.load_chart(info, chart);
        }

        let events: Vec<LiveEvent> = {
            let mut q = self.event_queue.borrow_mut();
            q.drain(..).collect()
        };
        for event in &events {
            match event {
                LiveEvent::Authenticate(Ok((info, room_state))) => {
                    console_log!(
                        "GameMonitor: authenticated as {} (id={}), room_state: {:?}",
                        info.name,
                        info.id,
                        room_state.as_ref().map(|s| format!("{:?}", s.state))
                    );
                    if let Some(state) = room_state {
                        if let RoomState::SelectChart(Some(id)) = state.state {
                            self.selected_chart_id = Some(id);
                            console_log!("GameMonitor: chart selected: {}", id);
                        }
                    }
                }
                LiveEvent::Authenticate(Err(e)) => {
                    console_log!("GameMonitor: auth failed: {}", e);
                }
                LiveEvent::Join(Ok(resp)) => {
                    console_log!("GameMonitor: joined room, {} users", resp.users.len());
                    // Create headless scenes for all users in the room
                    for user in &resp.users {
                        console_log!(
                            "  user: {} (id={}), monitor={}",
                            user.name,
                            user.id,
                            user.monitor
                        );
                        self.scenes
                            .entry(user.id)
                            .or_insert_with(|| GameScene::new_headless(user.id));
                    }
                    if let RoomState::SelectChart(Some(id)) = resp.state {
                        self.selected_chart_id = Some(id);
                        console_log!("GameMonitor: chart selected: {}", id);
                    }
                }
                LiveEvent::Join(Err(e)) => {
                    console_log!("GameMonitor: join failed: {}", e);
                }
                LiveEvent::Leave(r) => {
                    console_log!("GameMonitor: leave result: {:?}", r);
                    self.scenes.clear();
                    self.selected_chart_id = None;
                }
                LiveEvent::StateChange(state) => {
                    console_log!("GameMonitor: state change: {:?}", state);
                    if matches!(state, RoomState::Playing) {
                        self.start_all_scenes();
                    }
                    if matches!(state, RoomState::WaitingForReady) {
                        if let Some(id) = self.selected_chart_id {
                            console_log!("GameMonitor: fetching chart {} internally...", id);
                            let api_base = self.api_base.clone();
                            let ws = self.ws.clone();
                            let pending_chart = self.pending_chart.clone();

                            // Send custom binary command back to proxy...
                            // Actually, we'll spawn a local task to fetch the chart
                            // and then send the Ready command directly over WS.
                            spawn_local(async move {
                                if let Ok((info, chart)) =
                                    fetch_and_parse_chart(&api_base, id).await
                                {
                                    console_log!(
                                        "GameMonitor: chart {} loaded internally, sending Ready...",
                                        id
                                    );
                                    *pending_chart.borrow_mut() = Some((info, chart));

                                    let mut buf = Vec::new();
                                    encode_packet(&WsCommand::Ready, &mut buf);
                                    let _ = ws.send_with_u8_array(&buf);
                                } else {
                                    console_log!("GameMonitor: failed to load chart {}", id);
                                }
                            });
                        }
                    }
                }
                LiveEvent::UserJoin(info) => {
                    console_log!(
                        "GameMonitor: user joined: {} (id={}), monitor={}",
                        info.name,
                        info.id,
                        info.monitor
                    );
                    // Create headless scene for the new user
                    self.scenes
                        .entry(info.id)
                        .or_insert_with(|| GameScene::new_headless(info.id));
                }
                LiveEvent::UserLeave { user } => {
                    console_log!("GameMonitor: user left: id={}", user);
                    self.destroy_scene(*user);
                }
                LiveEvent::Touches { player, frames } => {
                    for f in frames {
                        console_log!("GameMonitor: TouchFrame received for #{player}: {f:?}");
                    }
                    if let Some(scene) = self.scenes.get_mut(player) {
                        scene.push_touches(frames);
                    }
                }
                LiveEvent::Judges { player, judges } => {
                    for j in judges {
                        console_log!("GameMonitor: JudgeEvent for #{player}: {j:?}");
                    }
                    if let Some(scene) = self.scenes.get_mut(player) {
                        scene.push_judges(judges);
                    }
                }
                LiveEvent::Message(msg) => {
                    console_log!("GameMonitor: message: {:?}", msg);
                    if let Message::SelectChart { id, .. } = msg {
                        self.selected_chart_id = Some(*id);
                        console_log!("GameMonitor: chart selected: {}", id);
                    }
                }
            }
        }

        // Render only scenes that have a canvas attached
        for scene in self.scenes.values_mut() {
            if scene.has_canvas() {
                scene.render(timestamp)?;
            }
        }

        Ok(())
    }

    /// Check if the WebSocket connection is still alive (CONNECTING or OPEN).
    /// Returns false only when the socket is CLOSING or CLOSED.
    pub fn is_connected(&self) -> bool {
        self.ws.ready_state() <= WebSocket::OPEN
    }

    /// Close the WebSocket connection.
    pub fn close(&self) -> Result<(), JsValue> {
        self.ws.close()
    }

    /// Load default texture resources into the specified GameScene's WebGL Context.
    pub async fn load_scene_resource_pack(
        &mut self,
        user_id: i32,
        files: js_sys::Object,
    ) -> Result<(), JsValue> {
        let entries = js_sys::Object::entries(&files);
        let mut file_map = std::collections::HashMap::new();

        for i in 0..entries.length() {
            let entry = entries.get(i);
            let entry_array = js_sys::Array::from(&entry);
            let key = entry_array.get(0).as_string().ok_or("Invalid key")?;
            let value = entry_array.get(1);
            let uint8_array = js_sys::Uint8Array::new(&value);
            file_map.insert(key, uint8_array.to_vec());
        }

        if let Some(scene) = self.scenes.get_mut(&user_id) {
            scene.load_resource_pack(file_map).await?;
        }

        Ok(())
    }

    /// Explicitly resume the browser audio context inside all active GameScenes.
    /// This is strictly required to bypass browser autoplay policies requiring a user gesture!
    pub fn resume_audio(&mut self) {
        for scene in self.scenes.values_mut() {
            scene.resume_audio_context();
        }
    }
}

impl GameMonitor {
    fn send_command(&self, cmd: &WsCommand) -> Result<(), JsValue> {
        let mut buf = Vec::new();
        encode_packet(cmd, &mut buf);
        self.ws.send_with_u8_array(&buf)
    }
}

async fn fetch_and_parse_chart(api_base: &str, id: i32) -> Result<(ChartInfo, Chart), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(
        window.fetch_with_str(&format!("{}/chart/{}", api_base, id)),
    )
    .await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;

    if !resp.ok() {
        return Err(JsValue::from_str(&format!(
            "Fetch failed: {}",
            resp.status_text()
        )));
    }

    let array_buffer = wasm_bindgen_futures::JsFuture::from(resp.array_buffer()?).await?;
    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    let vec = uint8_array.to_vec();

    use bincode::Options;
    let (info, mut chart): (ChartInfo, Chart) = bincode::options()
        .with_varint_encoding()
        .deserialize(&vec)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse chart: {}", e)))?;

    chart.order = (0..chart.lines.len()).collect();
    chart.order.sort_by_key(|&i| chart.lines[i].z_index);

    Ok((info, chart))
}
