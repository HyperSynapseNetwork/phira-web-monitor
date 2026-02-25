//! Standalone chart player — autoplay mode for the /play page.

use crate::audio::AudioEngine;
use crate::console_log;
use crate::engine::{ChartRenderer, JudgeEventKind, Resource, ResourcePack};
use crate::renderer::{Renderer, Texture};
use monitor_common::core::{
    Chart, ChartInfo, HitSound, JudgeLineKind, JudgeStatus, Judgement, NoteKind,
};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ChartPlayer {
    renderer: Renderer,
    chart_renderer: ChartRenderer,
    resource: Resource,
    audio_engine: AudioEngine,
    paused: bool,
    current_time: f32,
    last_update_time: Option<f64>,
    api_base: String,
}

#[wasm_bindgen]
impl ChartPlayer {
    fn sync_hitsounds(&mut self) -> Result<(), JsValue> {
        if let Some(pack) = &self.resource.res_pack {
            for (kind, clip) in &pack.hitsounds {
                self.audio_engine.set_hitsound(kind.clone(), clip)?;
            }
        }
        Ok(())
    }

    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: String, api_base: Option<String>) -> Result<ChartPlayer, JsValue> {
        console_error_panic_hook::set_once();
        let api_base = api_base.unwrap_or_default();
        console_log!(
            "ChartPlayer Initialized on Canvas '{}', API base: '{}'",
            canvas_id,
            api_base
        );

        let renderer = Renderer::new(&canvas_id)?;
        let mut resource = Resource::new(renderer.context.width, renderer.context.height);
        resource.load_defaults(&renderer.context)?;

        let info = ChartInfo::default();
        let chart = Chart::default();

        let mut player = ChartPlayer {
            renderer,
            chart_renderer: ChartRenderer::new(info, chart),
            resource,
            audio_engine: AudioEngine::new()?,
            paused: true,
            current_time: 0.0,
            last_update_time: None,
            api_base,
        };
        player.sync_hitsounds()?;
        Ok(player)
    }

    pub fn pause(&mut self) -> Result<(), JsValue> {
        self.paused = true;
        self.last_update_time = None;
        self.audio_engine.pause()
    }

    pub fn resume(&mut self) -> Result<(), JsValue> {
        self.paused = false;
        self.last_update_time = None;
        self.audio_engine.play(self.current_time)
    }

    pub fn set_time(&mut self, time: f32) {
        self.current_time = time;
        self.last_update_time = None;

        // Reset all judge states on seek
        for line in &mut self.chart_renderer.chart.lines {
            for note in &mut line.notes {
                note.judge = JudgeStatus::NotJudged;
            }
        }

        // Force update chart state immediately
        self.chart_renderer
            .update(&mut self.resource, self.current_time);
    }

    pub fn set_autoplay(&mut self, flag: bool) {
        self.chart_renderer.autoplay = flag;
    }

    pub fn render(&mut self) -> Result<(), JsValue> {
        let now = web_sys::window().unwrap().performance().unwrap().now();

        let mut dt = 0.0;
        if !self.paused {
            self.current_time = self.audio_engine.get_time();
            if let Some(last) = self.last_update_time {
                dt = (now - last) as f32 / 1000.0;
            }
            self.last_update_time = Some(now);
        }
        self.resource.dt = dt;

        self.renderer.clear();
        self.renderer.begin_frame();

        let aspect = self.resource.aspect_ratio;
        let y_scale = aspect;

        self.renderer.set_projection(&[
            1.0, 0.0, 0.0, 0.0, 0.0, y_scale, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);

        self.chart_renderer
            .update(&mut self.resource, self.current_time);

        let autoplay = self.chart_renderer.autoplay;
        let events = self
            .chart_renderer
            .update_judges(&self.resource, |chart, t| {
                let mut hook_events = Vec::new();
                for (line_idx, line) in chart.lines.iter_mut().enumerate() {
                    for (note_idx, note) in line.notes.iter_mut().enumerate() {
                        if note.fake {
                            continue;
                        }
                        if matches!(note.judge, JudgeStatus::NotJudged) {
                            if autoplay && note.time <= t {
                                match &note.kind {
                                    NoteKind::Hold { .. } => {
                                        note.judge =
                                            JudgeStatus::Hold(true, t, 0.0, false, f32::INFINITY);
                                        hook_events.push(crate::engine::JudgeEvent {
                                            kind: JudgeEventKind::HoldStart,
                                            line_idx,
                                            note_idx,
                                        });
                                    }
                                    _ => {
                                        note.judge = JudgeStatus::Judged(t, Judgement::Perfect);
                                        hook_events.push(crate::engine::JudgeEvent {
                                            kind: JudgeEventKind::Judged(Judgement::Perfect),
                                            line_idx,
                                            note_idx,
                                        });
                                    }
                                }
                            } else if !autoplay && t - note.time > 0.160 {
                                note.judge = JudgeStatus::Judged(t, Judgement::Miss);
                            }
                        }
                    }
                }
                hook_events
            });

        // Consume events: play hitsounds
        for event in &events {
            match &event.kind {
                JudgeEventKind::Judged(j) if matches!(j, Judgement::Miss | Judgement::Bad) => {}
                JudgeEventKind::Judged(_) | JudgeEventKind::HoldStart => {
                    let note =
                        &self.chart_renderer.chart.lines[event.line_idx].notes[event.note_idx];
                    let hitsound = note.hitsound.clone().unwrap_or_else(|| match note.kind {
                        NoteKind::Click => HitSound::Click,
                        NoteKind::Drag => HitSound::Drag,
                        NoteKind::Flick => HitSound::Flick,
                        _ => HitSound::Click,
                    });
                    let _ = self.audio_engine.play_hitsound(&hitsound);
                }
                _ => {}
            }
        }

        // Consume events: emit particles
        self.chart_renderer
            .emit_particles(&mut self.resource, &events);

        self.chart_renderer
            .render(&mut self.resource, &mut self.renderer);
        self.renderer.flush();
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
        self.resource.width = width;
        self.resource.height = height;
        self.resource.aspect_ratio = width as f32 / height as f32;
    }

    pub async fn load_chart(&mut self, id: String) -> Result<JsValue, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let resp_value = wasm_bindgen_futures::JsFuture::from(
            window.fetch_with_str(&format!("{}/chart/{}", self.api_base, id)),
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

        let existing_pack = self.resource.res_pack.take();
        let renderer = &self.renderer;
        let mut resource = Resource::new(renderer.context.width, renderer.context.height);
        resource.load_defaults(&renderer.context)?;

        if let Some(pack) = existing_pack {
            if pack.info.name != "fallback" {
                resource
                    .set_pack(&renderer.context, pack)
                    .map_err(|e| JsValue::from_str(&format!("Failed to restore pack: {}", e)))?;
            }
        }

        for (i, line) in chart.lines.iter().enumerate() {
            match &line.kind {
                JudgeLineKind::Texture(tex, _) => {
                    if let Ok(texture) =
                        Texture::load_from_bytes(&renderer.context, tex.data()).await
                    {
                        resource.line_textures.insert(i, texture);
                    }
                }
                JudgeLineKind::TextureGif(_, frames, _) => {
                    let mut gl_frames = Vec::new();
                    for (_time, tex) in &frames.frames {
                        if let Ok(texture) =
                            Texture::load_from_bytes(&renderer.context, tex.data()).await
                        {
                            gl_frames.push(texture);
                        }
                    }
                    resource.line_gif_textures.insert(i, gl_frames);
                }
                _ => {}
            }
        }

        let autoplay = self.chart_renderer.autoplay;
        self.chart_renderer = ChartRenderer::new(info.clone(), chart);
        self.chart_renderer.autoplay = autoplay;
        self.resource = resource;
        self.current_time = 0.0;
        self.paused = true;
        self.last_update_time = None;

        // Load Audio into Engine
        self.audio_engine.pause()?;
        self.audio_engine
            .set_offset(self.chart_renderer.chart.offset);

        if let Some(music) = &self.chart_renderer.chart.music {
            self.audio_engine.set_music(music)?;
        }

        // 1. Sync default hitsounds from resource pack
        self.sync_hitsounds()?;

        // 2. Override with chart-specific hitsounds if any
        for (kind, clip) in &self.chart_renderer.chart.hitsounds {
            self.audio_engine.set_hitsound(kind.clone(), clip)?;
        }

        serde_wasm_bindgen::to_value(&info)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize chart info: {}", e)))
    }

    pub async fn load_resource_pack(&mut self, files: js_sys::Object) -> Result<(), JsValue> {
        let entries = js_sys::Object::entries(&files);
        let mut file_map = HashMap::new();

        for i in 0..entries.length() {
            let entry = entries.get(i);
            let entry_array = js_sys::Array::from(&entry);
            let key = entry_array.get(0).as_string().ok_or("Invalid key")?;
            let value = entry_array.get(1);
            let uint8_array = js_sys::Uint8Array::new(&value);
            file_map.insert(key, uint8_array.to_vec());
        }

        let res_pack = ResourcePack::load(&self.renderer.context, file_map)
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to load pack: {:?}", e)))?;

        self.resource
            .set_pack(&self.renderer.context, res_pack)
            .map_err(|e| JsValue::from_str(&format!("Failed to set pack: {}", e)))?;

        self.sync_hitsounds()?;

        Ok(())
    }
}
