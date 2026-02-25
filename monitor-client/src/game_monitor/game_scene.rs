//! Per-player rendering context for live monitoring.

use std::collections::VecDeque;

use crate::{
    audio::AudioEngine,
    console_log,
    engine::{ChartRenderer, JudgeEventKind, Resource},
    renderer::Renderer,
};
use monitor_common::core::{Chart, ChartInfo, HitSound, JudgeStatus, Judgement, NoteKind};
use phira_mp_common::TouchFrame;
use wasm_bindgen::prelude::*;

// ── Touch overlay constants ─────────────────────────────────────────────────

const TOUCH_COLORS: &[[f32; 3]] = &[
    [0.3, 0.6, 1.0],
    [1.0, 0.4, 0.4],
    [0.3, 1.0, 0.5],
    [1.0, 0.8, 0.2],
    [0.8, 0.4, 1.0],
    [1.0, 0.6, 0.3],
];
const TOUCH_RADIUS: f32 = 0.015;
const TOUCH_ALPHA: f32 = 0.5;
const TOUCH_FADE_TIME: f32 = 0.3;

/// A currently active or fading touch point.
struct ActiveTouch {
    finger_id: i8,
    x: f32,
    y: f32,
    color: [f32; 3],
    /// None = currently pressed, Some(remaining) = fading out
    fade_timer: Option<f32>,
}

/// Per-player rendering context for live monitoring.
///
/// Each `GameScene` owns its own Renderer, ChartRenderer, Resource, and
/// AudioEngine. Judges and touches are pushed into pending buffers by
/// `Monitor::tick()` and consumed during `render()`.
pub struct GameScene {
    pub user_id: i32,
    renderer: Renderer,
    chart_renderer: ChartRenderer,
    resource: Resource,
    audio_engine: AudioEngine,
    current_time: f32,
    last_render_time: Option<f64>,
    started: bool,
    start_time: Option<f64>,
    paused_for_judge: bool,
    paused_time: f32,
    audio_playing: bool,
    pub target_time: Option<f32>,
    pub unpause_signal: Option<f32>,

    // MP event buffers
    pub pending_judges: VecDeque<phira_mp_common::JudgeEvent>,
    touch_points: Vec<ActiveTouch>,
}

impl GameScene {
    /// Create a new scene for the given user, bound to a `<canvas>` element.
    pub fn new(user_id: i32, canvas_id: &str) -> Result<Self, JsValue> {
        let renderer = Renderer::new(canvas_id)?;
        let mut resource = Resource::new(renderer.context.width, renderer.context.height);
        resource.load_defaults(&renderer.context)?;

        Ok(GameScene {
            user_id,
            renderer,
            chart_renderer: ChartRenderer::new(ChartInfo::default(), Chart::default()),
            resource,
            audio_engine: AudioEngine::new()?,
            current_time: 0.0,
            last_render_time: None,
            started: false,
            start_time: None,
            paused_for_judge: false,
            paused_time: 0.0,
            audio_playing: false,
            target_time: None,
            unpause_signal: None,

            pending_judges: VecDeque::new(),
            touch_points: Vec::new(),
        })
    }

    /// Load a pre-parsed chart into this scene.
    pub fn load_chart(&mut self, info: ChartInfo, chart: Chart) {
        self.chart_renderer = ChartRenderer::new(info, chart);
        self.chart_renderer.autoplay = false;
        self.current_time = 0.0;
        self.last_render_time = None;
        self.started = false;
        self.start_time = None;
        self.paused_for_judge = false;
        self.paused_time = 0.0;
        self.audio_playing = false;
        self.target_time = None;
        self.unpause_signal = None;
        self.pending_judges.clear();
        self.touch_points.clear();

        // Safely parse audio offsets and chart-specific music / hitsounds
        let _ = self.audio_engine.pause();
        self.audio_engine
            .set_offset(self.chart_renderer.chart.offset);

        if let Some(music) = &self.chart_renderer.chart.music {
            let _ = self.audio_engine.set_music(music);
        }

        for (kind, clip) in &self.chart_renderer.chart.hitsounds {
            let _ = self.audio_engine.set_hitsound(kind.clone(), clip);
        }

        console_log!("GameScene[{}]: chart loaded", self.user_id);
    }

    /// Clear the scene, blanking the canvas and discarding the chart.
    pub fn clear(&mut self) {
        self.started = false;
        use monitor_common::core::{Chart, ChartInfo};
        self.chart_renderer = ChartRenderer::new(ChartInfo::default(), Chart::default());
        self.current_time = 0.0;
        self.last_render_time = None;
        self.start_time = None;
        self.paused_for_judge = false;
        self.paused_time = 0.0;
        self.audio_playing = false;
        self.target_time = None;
        self.unpause_signal = None;
        self.pending_judges.clear();
        self.touch_points.clear();
        console_log!("GameScene[{}]: cleared", self.user_id);
    }

    /// Load default texture resources into the scene's independent WebGL context
    pub async fn load_resource_pack(
        &mut self,
        file_map: std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<(), JsValue> {
        use crate::engine::ResourcePack;
        let res_pack = ResourcePack::load(&self.renderer.context, file_map)
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to load pack: {:?}", e)))?;
        self.resource
            .set_pack(&self.renderer.context, res_pack)
            .map_err(|e| JsValue::from_str(&format!("Failed to set pack: {}", e)))?;
        console_log!("GameScene[{}]: resource pack loaded", self.user_id);

        // Synchronize default hitsounds directly from the loaded pack
        if let Some(pack) = &self.resource.res_pack {
            for (kind, clip) in &pack.hitsounds {
                let _ = self.audio_engine.set_hitsound(kind.clone(), clip);
            }
        }

        Ok(())
    }

    /// Explicitly resume the WebAudio AudioContext to bypass browser autoplay policies.
    pub fn resume_audio_context(&mut self) {
        // web_sys::AudioContext::resume() returns a Promise.
        // We evaluate it simply by triggering it.
        let _ = self.audio_engine.play(0.0).ok();
        let _ = self.audio_engine.pause().ok();
        console_log!(
            "GameScene[{}]: explicit audio context resume request sent",
            self.user_id
        );
    }

    /// Begin chart playback (called when room state transitions to Playing).
    pub fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.start_time = None; // Will be set on first render
        self.current_time = 0.0;
        self.last_render_time = None;
        // Don't play audio immediately; wait for start delay
        self.audio_playing = false;
        console_log!("GameScene[{}]: started", self.user_id);
    }

    pub fn clear_stale_notes(&mut self, player_time: f32) {
        self.chart_renderer.clear_stale_notes(player_time);
    }

    /// Ingest touch frames into the active touch list.
    pub fn push_touches(&mut self, frames: &[TouchFrame]) {
        if let Some(last) = frames.last() {
            let t = last.time;
            self.target_time = Some(self.target_time.unwrap_or(t).max(t));
        }

        for frame in frames {
            // Mark existing touches not present in this frame as released
            let mut seen: Vec<i8> = Vec::new();
            for &(finger_id, ref pos) in &frame.points {
                seen.push(finger_id);
                let x = pos.x();
                let y = pos.y();
                // Find existing or create new
                if let Some(touch) = self
                    .touch_points
                    .iter_mut()
                    .find(|t| t.finger_id == finger_id)
                {
                    touch.x = x;
                    touch.y = y;
                    touch.fade_timer = None; // still pressed
                } else {
                    let color_idx = (finger_id.unsigned_abs() as usize) % TOUCH_COLORS.len();
                    self.touch_points.push(ActiveTouch {
                        finger_id,
                        x,
                        y,
                        color: TOUCH_COLORS[color_idx],
                        fade_timer: None,
                    });
                }
            }
            // Start fade for touches not in this frame
            for touch in &mut self.touch_points {
                if !seen.contains(&touch.finger_id) && touch.fade_timer.is_none() {
                    touch.fade_timer = Some(TOUCH_FADE_TIME);
                }
            }
        }
    }

    /// Full render pass. `now` is `performance.now()` in milliseconds.
    pub fn render(&mut self, now: f64) -> Result<(), JsValue> {
        if !self.started {
            self.renderer.clear();
            self.renderer.flush();
            return Ok(());
        }

        // Compute dt
        let mut real_dt = 0.0f32;
        if let Some(last) = self.last_render_time {
            real_dt = ((now - last) / 1000.0) as f32;
        }
        self.last_render_time = Some(now);

        let mut chart_dt = real_dt;

        // Rule 1: Start Delay
        if self.start_time.is_none() {
            self.start_time = Some(now);
        }
        if let Some(st) = self.start_time {
            if now - st < 5000.0 {
                // Wait 5.0s before playing
                self.current_time = 0.0;
                self.resource.dt = 0.0;
                return Ok(());
            } else if !self.audio_playing && !self.paused_for_judge {
                // Time to start!
                let _ = self.audio_engine.play(0.0);
                self.audio_playing = true;
            }
        }

        // Rule 2 & 3: Strict Pause & Resume with Delta
        if let Some(_ev_time) = self.unpause_signal.take() {
            if self.paused_for_judge {
                // Rule 3: Resume with delta 1.000s from pause_time
                self.paused_for_judge = false;
                self.current_time = (self.paused_time - 1.000).max(0.0);

                // Only clear notes that are TRULY stale relative to our new rewound time.
                // This correctly avoids visually destroying notes between `current_time` and `ev_time`.
                self.chart_renderer.clear_stale_notes(self.current_time);

                // Synchronize audio engine with rewound time
                let _ = self.audio_engine.play(self.current_time.into());
                self.audio_playing = true;

                console_log!(
                    "GameMonitor: GameScene[{}]: resuming from {:.3} (paused at: {:.3})",
                    self.user_id,
                    self.current_time,
                    self.paused_time
                );
            }
        }

        if self.paused_for_judge {
            self.current_time = self.paused_time;
            chart_dt = 0.0;
        } else {
            self.current_time = self.audio_engine.get_time();
        }

        self.resource.dt = chart_dt;

        self.renderer.clear();
        self.renderer.begin_frame();

        let aspect = self.resource.aspect_ratio;
        self.renderer.set_projection(&[
            1.0, 0.0, 0.0, 0.0, 0.0, aspect, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);

        // Update chart animations
        self.chart_renderer
            .update(&mut self.resource, self.current_time);

        // Apply MP judge events via the hook
        let pending_judges = &mut self.pending_judges;
        let all_events = self
            .chart_renderer
            .update_judges(&self.resource, |chart, t| {
                let mut hook_events = Vec::new();
                while let Some(ev) = pending_judges.front() {
                    if ev.time > t {
                        break;
                    }
                    let ev = pending_judges.pop_front().unwrap();
                    let Some(line) = chart.lines.get_mut(ev.line_id as usize) else {
                        continue;
                    };
                    let Some(note) = line.notes.get_mut(ev.note_id as usize) else {
                        continue;
                    };

                    let line_idx = ev.line_id as usize;
                    let note_idx = ev.note_id as usize;

                    use phira_mp_common::Judgement as MpJudgement;
                    match ev.judgement {
                        MpJudgement::Perfect
                        | MpJudgement::Good
                        | MpJudgement::Bad
                        | MpJudgement::Miss => {
                            let j = match ev.judgement {
                                MpJudgement::Perfect => Judgement::Perfect,
                                MpJudgement::Good => Judgement::Good,
                                MpJudgement::Bad => Judgement::Bad,
                                _ => Judgement::Miss,
                            };
                            note.judge = JudgeStatus::Judged(ev.time, j);
                            hook_events.push(crate::engine::JudgeEvent {
                                kind: crate::engine::JudgeEventKind::Judged(j),
                                line_idx,
                                note_idx,
                            });
                        }
                        MpJudgement::HoldPerfect => {
                            note.judge = JudgeStatus::Hold(true, t, 0.0, false, f32::INFINITY);
                            hook_events.push(crate::engine::JudgeEvent {
                                kind: crate::engine::JudgeEventKind::HoldStart,
                                line_idx,
                                note_idx,
                            });
                        }
                        MpJudgement::HoldGood => {
                            note.judge = JudgeStatus::Hold(false, t, 0.0, false, f32::INFINITY);
                            hook_events.push(crate::engine::JudgeEvent {
                                kind: crate::engine::JudgeEventKind::HoldStart,
                                line_idx,
                                note_idx,
                            });
                        }
                    }
                }
                hook_events
            });

        // Play hitsounds
        for event in &all_events {
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

        // Emit particles
        self.chart_renderer
            .emit_particles(&mut self.resource, &all_events);

        // Render chart
        self.chart_renderer
            .render(&mut self.resource, &mut self.renderer);

        // Render touch overlay
        self.render_touches(real_dt);

        // Rule 2: Strict Pause
        // Check if we need to pause FOR THE NEXT FRAME, AFTER processing all events up to current_time
        if !self.paused_for_judge && self.chart_renderer.has_unjudged(self.current_time) {
            self.paused_for_judge = true;
            self.paused_time = self.current_time;
            let _ = self.audio_engine.pause();
            self.audio_playing = false;
            console_log!(
                "GameMonitor: GameScene[{}]: paused for judge at {:.3} (limit: 0.200s exceeded)",
                self.user_id,
                self.current_time
            );
        }

        self.renderer.flush();
        Ok(())
    }

    /// Resize the scene's canvas.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
        self.resource.width = width;
        self.resource.height = height;
        self.resource.aspect_ratio = width as f32 / height as f32;
    }

    // ── Touch overlay rendering ─────────────────────────────────────────

    fn render_touches(&mut self, dt: f32) {
        let aspect = self.resource.aspect_ratio;

        // Identity model matrix for overlay drawing
        const IDENTITY: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        // Update fade timers and remove expired touches
        self.touch_points.retain_mut(|touch| {
            if let Some(ref mut timer) = touch.fade_timer {
                *timer -= dt;
                if *timer <= 0.0 {
                    return false; // remove
                }
            }
            true
        });

        // Draw remaining touches
        for touch in &self.touch_points {
            let alpha = if let Some(timer) = touch.fade_timer {
                TOUCH_ALPHA * (timer / TOUCH_FADE_TIME)
            } else {
                TOUCH_ALPHA
            };

            let [r, g, b] = touch.color;
            // CompactPos stores (x, y * aspect), so divide y by aspect for rendering
            let screen_y = touch.y / aspect;
            self.renderer.draw_rect(
                touch.x - TOUCH_RADIUS,
                screen_y - TOUCH_RADIUS,
                TOUCH_RADIUS * 2.0,
                TOUCH_RADIUS * 2.0,
                r,
                g,
                b,
                alpha,
                &IDENTITY,
            );
        }
    }
}
