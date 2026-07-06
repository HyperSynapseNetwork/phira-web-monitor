//! Live monitoring client — WebSocket connection and event dispatcher.

mod game_scene;
pub use game_scene::GameScene;

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

use crate::chart_asset::{fetch_and_parse_chart, file_map_from_js_object};
use crate::engine::{ChartRenderer, LoadedLineTextures};
use monitor_common::{
    core::{Chart, ChartInfo},
    live::{LiveEvent, WsCommand},
};
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

    // Pending line texture load results (from spawn_local)
    #[wasm_bindgen(skip)]
    completed_line_textures: Rc<RefCell<Vec<LineTextureJobResult>>>,
}

struct LineTextureJobResult {
    user_id: i32,
    result: Result<LoadedLineTextures, JsValue>,
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
            completed_line_textures: Rc::new(RefCell::new(Vec::new())),
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
    pub async fn attach_canvas(&mut self, user_id: i32, canvas_id: &str) -> Result<(), JsValue> {
        // Ensure a headless scene exists
        self.scenes
            .entry(user_id)
            .or_insert_with(|| GameScene::new_headless(user_id));

        let scene = self.scenes.get_mut(&user_id).unwrap();

        // Immediately load chart if we already have it and scene doesn't have one
        if let (Some(info), Some(data)) = (&self.chart_info, &self.chart_data)
            && !scene.has_chart()
        {
            scene.load_chart(info.clone(), data.clone());
        }

        scene.attach_canvas(canvas_id).await?;
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
            if let Some((ctx, chart)) = scene.take_line_texture_job() {
                spawn_line_texture_job(self.completed_line_textures.clone(), *uid, ctx, chart);
            }
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

        let completed_texture_jobs: Vec<_> = self
            .completed_line_textures
            .borrow_mut()
            .drain(..)
            .collect();
        for job in completed_texture_jobs {
            let Some(scene) = self.scenes.get_mut(&job.user_id) else {
                continue;
            };
            match job.result {
                Ok(loaded) => scene.apply_line_textures(loaded),
                Err(err) => {
                    scene.mark_line_textures_pending();
                    console_log!(
                        "GameMonitor: line texture load failed for #{}: {:?}",
                        job.user_id,
                        err
                    );
                }
            }
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
                    if let Some(state) = room_state
                        && let RoomState::SelectChart(Some(id)) = state.state
                    {
                        self.selected_chart_id = Some(id);
                        console_log!("GameMonitor: chart selected: {}", id);
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
                    if matches!(state, RoomState::WaitingForReady)
                        && let Some(id) = self.selected_chart_id
                    {
                        console_log!("GameMonitor: fetching chart {} internally...", id);
                        let api_base = self.api_base.clone();
                        let ws = self.ws.clone();
                        let pending_chart = self.pending_chart.clone();

                        // Send custom binary command back to proxy...
                        // Actually, we'll spawn a local task to fetch the chart
                        // and then send the Ready command directly over WS.
                        spawn_local(async move {
                            if let Ok((info, chart)) = fetch_and_parse_chart(&api_base, id).await {
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
                    if let Some(scene) = self.scenes.get_mut(player) {
                        scene.push_touches(frames);
                    }
                }
                LiveEvent::Judges { player, judges } => {
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
        let file_map = file_map_from_js_object(files)?;
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

fn spawn_line_texture_job(
    completed: Rc<RefCell<Vec<LineTextureJobResult>>>,
    user_id: i32,
    ctx: crate::renderer::GlContext,
    chart: Chart,
) {
    spawn_local(async move {
        let result = ChartRenderer::load_line_texture_maps(&ctx, &chart).await;
        completed
            .borrow_mut()
            .push(LineTextureJobResult { user_id, result });
    });
}
