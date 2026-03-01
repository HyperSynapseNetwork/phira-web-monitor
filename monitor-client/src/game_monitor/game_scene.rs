//! Per-player rendering context for live monitoring.

use std::collections::{HashMap, VecDeque};

use crate::{
    audio::AudioEngine,
    console_log,
    engine::{ChartRenderer, JudgeEventKind, Resource},
    renderer::Renderer,
    time::TimeManager,
};
use monitor_common::core::{
    AnimVector, Chart, ChartInfo, HitSound, JudgeStatus, Judgement, Keyframe, NoteKind,
};
use phira_mp_common::{JudgeEvent, TouchFrame};
use wasm_bindgen::prelude::*;

// ── Touch overlay constants ─────────────────────────────────────────────────

const TOUCH_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
const TOUCH_RADIUS: f32 = 0.015;
const TOUCH_ALPHA: f32 = 0.6;
const TOUCH_FADE_TIME: f32 = 0.3;
/// Offset subtracted from target_time when seeking on canvas attach (seconds)
const SEEK_OFFSET: f32 = 0.1;
/// Start delay duration in seconds
const START_DELAY_SECS: f64 = 4.5;

// ── ActiveTouch ─────────────────────────────────────────────────────────────

struct ActiveTouch {
    anim: AnimVector,
    start_time: f32,
    last_update: f32,
    /// None = currently pressed, Some(game_time) = touch ended at this time
    end_time: Option<f32>,
}

// ── RenderContext ───────────────────────────────────────────────────────────

/// Heavy hardware-bound rendering state. Created lazily when a canvas is
/// attached, dropped when the canvas is detached.
pub struct RenderContext {
    pub renderer: Renderer,
    pub resource: Resource,
    pub audio_engine: AudioEngine,
}

// ── GameScene ───────────────────────────────────────────────────────────────

/// Per-player game state for live monitoring.
///
/// Created headlessly for every player when the room is joined.
/// Rendering components (`RenderContext`) are lazily initialized when a
/// canvas is attached via `attach_canvas()`.
pub struct GameScene {
    pub user_id: i32,

    // Lazily initialized rendering context (WebGL + Audio)
    render_ctx: Option<RenderContext>,
    // Lazily initialized chart renderer (persists across attach/detach)
    chart_renderer: Option<ChartRenderer>,

    // Core simulation state
    time: TimeManager,
    /// If `Some(t)`, the scene is paused waiting for a judge event at game time `t`.
    judge_pause_time: Option<f32>,
    target_time: Option<f32>,
    unpause_signal: Option<f32>,

    // Wall-clock time (ms, from performance.now()) when start() was called
    start_wall_time: Option<f64>,

    // MP event buffers
    pending_judges: VecDeque<phira_mp_common::JudgeEvent>,
    /// Currently pressed touches, keyed by finger_id
    active_touches: HashMap<i8, ActiveTouch>,
    /// Touches that have been released and are fading out
    fading_touches: Vec<ActiveTouch>,
}

impl GameScene {
    /// Create a headless scene for the given user (no WebGL, no audio).
    /// Events will be buffered until `attach_canvas()` is called.
    pub fn new_headless(user_id: i32) -> Self {
        let mut time = TimeManager::new();
        time.pause(); // Paused until start() is called
        GameScene {
            user_id,
            render_ctx: None,
            chart_renderer: None,
            time,
            judge_pause_time: None,
            target_time: None,
            unpause_signal: None,
            start_wall_time: None,
            pending_judges: VecDeque::new(),
            active_touches: HashMap::new(),
            fading_touches: Vec::new(),
        }
    }

    /// Reset all simulation state to initial values.
    fn reset_state(&mut self) {
        self.time.reset();
        self.time.pause();
        self.judge_pause_time = None;
        self.target_time = None;
        self.unpause_signal = None;
        self.start_wall_time = None;
        self.pending_judges.clear();
        self.active_touches.clear();
        self.fading_touches.clear();
    }

    /// Synchronize audio engine with the current chart's music and hitsounds.
    fn sync_audio(&mut self) {
        if let (Some(ctx), Some(cr)) = (&mut self.render_ctx, &self.chart_renderer) {
            ctx.audio_engine.set_offset(cr.chart.offset);
            if let Some(music) = &cr.chart.music {
                let _ = ctx.audio_engine.set_music(music);
            }
            for (kind, clip) in &cr.chart.hitsounds {
                let _ = ctx.audio_engine.set_hitsound(kind.clone(), clip);
            }
        }
    }

    /// Attach a `<canvas>` element to this scene, initializing WebGL and Audio.
    /// If already attached, this is a no-op.
    pub fn attach_canvas(&mut self, canvas_id: &str) -> Result<(), JsValue> {
        if self.render_ctx.is_some() {
            return Ok(());
        }

        let renderer = Renderer::new(canvas_id)?;
        let mut resource = Resource::new(renderer.context.width, renderer.context.height);
        resource.load_defaults(&renderer.context)?;
        let audio_engine = AudioEngine::new()?;

        self.render_ctx = Some(RenderContext {
            renderer,
            resource,
            audio_engine,
        });

        // If chart is already loaded, sync audio
        self.sync_audio();

        // If already started, seek to tracked game time so rendering picks up mid-game
        if self.start_wall_time.is_some() {
            if let Some(target) = self.target_time {
                let seek_pos = (target - SEEK_OFFSET).max(0.0);
                self.time.seek_to(seek_pos as f64);
                console_log!(
                    "GameScene[{}]: mid-game attach, seeking to {:.3}s (target={:.3})",
                    self.user_id,
                    seek_pos,
                    target
                );
            }
        }

        console_log!("GameScene[{}]: canvas attached", self.user_id);
        Ok(())
    }

    /// Detach the canvas, freeing WebGL context and AudioEngine.
    /// The headless state (chart_renderer, event buffers, time) is preserved.
    pub fn detach_canvas(&mut self) {
        if let Some(mut ctx) = self.render_ctx.take() {
            let _ = ctx.audio_engine.pause();
        }
        console_log!("GameScene[{}]: canvas detached", self.user_id);
    }

    /// Returns true if a rendering context is currently attached.
    pub fn has_canvas(&self) -> bool {
        self.render_ctx.is_some()
    }

    /// Returns true if a chart is currently loaded.
    pub fn has_chart(&self) -> bool {
        self.chart_renderer.is_some()
    }

    /// Load a pre-parsed chart into this scene.
    pub fn load_chart(&mut self, info: ChartInfo, chart: Chart) {
        let mut cr = ChartRenderer::new(info, chart);
        cr.autoplay = false;
        self.chart_renderer = Some(cr);

        self.reset_state();

        // Pause audio and sync with the new chart
        if let Some(ctx) = &mut self.render_ctx {
            let _ = ctx.audio_engine.pause();
        }
        self.sync_audio();

        console_log!("GameScene[{}]: chart loaded", self.user_id);
    }

    /// Clear the scene, discarding chart and resetting state.
    pub fn clear(&mut self) {
        self.chart_renderer = None;
        self.reset_state();
        console_log!("GameScene[{}]: cleared", self.user_id);
    }

    /// Load default texture resources into the scene's WebGL context.
    pub async fn load_resource_pack(
        &mut self,
        file_map: std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<(), JsValue> {
        let ctx = self
            .render_ctx
            .as_mut()
            .ok_or_else(|| JsValue::from_str("No render context attached"))?;

        use crate::engine::ResourcePack;
        let res_pack = ResourcePack::load(&ctx.renderer.context, file_map)
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to load pack: {:?}", e)))?;
        ctx.resource
            .set_pack(&ctx.renderer.context, res_pack)
            .map_err(|e| JsValue::from_str(&format!("Failed to set pack: {}", e)))?;
        console_log!("GameScene[{}]: resource pack loaded", self.user_id);

        // Synchronize default hitsounds from the loaded pack
        if let Some(pack) = &ctx.resource.res_pack {
            for (kind, clip) in &pack.hitsounds {
                let _ = ctx.audio_engine.set_hitsound(kind.clone(), clip);
            }
        }

        Ok(())
    }

    /// Explicitly resume the WebAudio AudioContext to bypass browser autoplay policies.
    pub fn resume_audio_context(&mut self) {
        if let Some(ctx) = &mut self.render_ctx {
            let _ = ctx.audio_engine.play(0.0).ok();
            let _ = ctx.audio_engine.pause().ok();
            console_log!(
                "GameScene[{}]: explicit audio context resume request sent",
                self.user_id
            );
        }
    }

    /// Begin chart playback (called when room state transitions to Playing).
    pub fn start(&mut self) {
        if self.start_wall_time.is_some() {
            return;
        }
        self.time.reset();
        self.time.pause(); // Will resume after start delay
        self.start_wall_time = Some(TimeManager::real_time_secs() * 1000.0);
        console_log!("GameScene[{}]: started", self.user_id);
    }

    pub fn clear_stale_notes(&mut self, player_time: f32) {
        if let Some(cr) = &mut self.chart_renderer {
            cr.clear_stale_notes(player_time);
        }
    }

    pub fn push_judges(&mut self, judges: &[JudgeEvent]) {
        self.pending_judges.extend(judges.iter().cloned());
        if let Some(last) = judges.last() {
            self.unpause_signal = Some(last.time);
            self.target_time = Some(self.target_time.unwrap_or(last.time).max(last.time));
        }
    }

    pub fn push_touches(&mut self, frames: &[TouchFrame]) {
        if let Some(last) = frames.last() {
            let t = last.time;
            self.target_time = Some(self.target_time.unwrap_or(t).max(t));
        }
        for frame in frames {
            for &(finger_id, ref pos) in &frame.points {
                if finger_id < 0 {
                    // Phira bit-inverts ID (!id) to signal touch end/cancel.
                    let real_id = !finger_id;
                    if let Some(mut touch) = self.active_touches.remove(&real_id) {
                        touch.end_time = Some(frame.time);
                        self.fading_touches.push(touch);
                    }
                } else {
                    let x = pos.x();
                    let y = pos.y();
                    let touch =
                        self.active_touches
                            .entry(finger_id)
                            .or_insert_with(|| ActiveTouch {
                                anim: AnimVector::default(),
                                start_time: frame.time,
                                last_update: frame.time,
                                end_time: None,
                            });
                    touch.anim.x.keyframes.push(Keyframe::new(frame.time, x, 2));
                    touch.anim.y.keyframes.push(Keyframe::new(frame.time, y, 2));
                    touch.last_update = frame.time;
                    touch.end_time = None;
                }
            }
        }
    }

    /// Full render pass. `now` is `performance.now()` in milliseconds.
    pub fn render(&mut self, now: f64) -> Result<(), JsValue> {
        // If no render context, nothing to draw
        let ctx = match &mut self.render_ctx {
            Some(ctx) => ctx,
            None => return Ok(()),
        };

        if self.start_wall_time.is_none() {
            ctx.renderer.clear();
            ctx.renderer.flush();
            return Ok(());
        }

        // Rule 1: Smart Start Delay
        // Normally wait START_DELAY_SECS after game start. But if we have target_time
        // (i.e. mid-game join with buffered events), skip straight ahead.
        if let Some(start_wall) = self.start_wall_time {
            let delay_ms = START_DELAY_SECS * 1000.0;
            let deadline = start_wall + delay_ms;

            // If we have buffered events, cap the deadline at the wall-clock
            // time corresponding to target_time so we don't wait needlessly.
            let effective_deadline = if let Some(target) = self.target_time {
                let target_wall = start_wall + (target as f64 * 1000.0);
                deadline.min(target_wall)
            } else {
                deadline
            };

            if now < effective_deadline {
                ctx.resource.dt = 0.0;
                return Ok(());
            } else if self.time.paused() && self.judge_pause_time.is_none() {
                // Delay elapsed, start the clock!
                if let Some(target) = self.target_time {
                    let seek_pos = (target - SEEK_OFFSET).max(0.0);
                    self.time.seek_to(seek_pos as f64);
                    self.time.resume();
                    let _ = ctx.audio_engine.play(seek_pos.into());
                } else {
                    self.time.resume();
                    let _ = ctx.audio_engine.play(0.0);
                }
            }
        } else {
            return Ok(());
        }

        // Rule 2 & 3: Strict Pause & Resume with Delta
        if let Some(_ev_time) = self.unpause_signal.take() {
            if let Some(paused_time) = self.judge_pause_time.take() {
                let resume_time = (paused_time - 1.000).max(0.0);
                self.time.seek_to(resume_time as f64);
                self.time.resume();

                if let Some(cr) = &mut self.chart_renderer {
                    cr.clear_stale_notes(resume_time);
                }

                let _ = ctx.audio_engine.play(resume_time.into());

                console_log!(
                    "GameMonitor: GameScene[{}]: resuming from {:.3} (paused at: {:.3})",
                    self.user_id,
                    resume_time,
                    paused_time
                );
            }
        }

        let current_time = self.judge_pause_time.unwrap_or_else(|| self.time.now());

        if self.judge_pause_time.is_some() {
            ctx.resource.dt = 0.0;
        }

        ctx.renderer.clear();
        ctx.renderer.begin_frame();

        let aspect = ctx.resource.aspect_ratio;
        ctx.renderer.set_projection(&[
            1.0, 0.0, 0.0, 0.0, 0.0, aspect, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);

        // Only render chart if chart_renderer exists
        if let Some(cr) = &mut self.chart_renderer {
            // Update chart animations
            cr.update(&mut ctx.resource, current_time);

            // Apply MP judge events via the hook
            let pending_judges = &mut self.pending_judges;
            let all_events = cr.update_judges(&ctx.resource, |chart, t| {
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
                        let note = &cr.chart.lines[event.line_idx].notes[event.note_idx];
                        let hitsound = note.hitsound.clone().unwrap_or_else(|| match note.kind {
                            NoteKind::Click | NoteKind::Hold { .. } => HitSound::Click,
                            NoteKind::Drag => HitSound::Drag,
                            NoteKind::Flick => HitSound::Flick,
                        });
                        let _ = ctx.audio_engine.play_hitsound(&hitsound);
                    }
                    _ => {}
                }
            }

            // Emit particles
            cr.emit_particles(&mut ctx.resource, &all_events);

            // Render chart
            cr.render(&mut ctx.resource, &mut ctx.renderer);

            // Rule 2: Strict Pause
            if self.judge_pause_time.is_none() && cr.has_unjudged(current_time) {
                self.judge_pause_time = Some(current_time);
                self.time.pause();
                let _ = ctx.audio_engine.pause();
                console_log!(
                    "GameMonitor: GameScene[{}]: paused for judge at {:.3}",
                    self.user_id,
                    current_time
                );
            }
        }

        // Render touch overlay
        self.render_touches(current_time);

        if let Some(ctx) = &mut self.render_ctx {
            ctx.renderer.flush();
        }
        Ok(())
    }

    /// Resize the scene's canvas.
    pub fn resize(&mut self, width: u32, height: u32) {
        let screen_ratio = width as f32 / height as f32;
        let design_ratio = self
            .chart_renderer
            .as_ref()
            .map(|cr| cr.info.aspect_ratio)
            .unwrap_or(16.0 / 9.0);

        // Cap at design ratio (match Phira's non-fix mode)
        let aspect_ratio = design_ratio.min(screen_ratio);

        // Compute letterboxed viewport
        let (vp_w, vp_h) = if screen_ratio > aspect_ratio {
            let vp_w = (height as f32 * aspect_ratio).round() as u32;
            (vp_w, height)
        } else {
            let vp_h = (width as f32 / aspect_ratio).round() as u32;
            (width, vp_h)
        };

        // Center the viewport
        let vp_x = (width - vp_w) / 2;
        let vp_y = (height - vp_h) / 2;

        if let Some(ctx) = &mut self.render_ctx {
            ctx.renderer.resize(width, height);
            ctx.renderer
                .set_viewport(vp_x as i32, vp_y as i32, vp_w, vp_h);
            ctx.resource.width = width;
            ctx.resource.height = height;
            ctx.resource.aspect_ratio = aspect_ratio;
        }
    }

    // ── Touch overlay rendering ─────────────────────────────────────────

    fn render_touches(&mut self, t: f32) {
        let ctx = match &mut self.render_ctx {
            Some(ctx) => ctx,
            None => return,
        };
        let aspect = ctx.resource.aspect_ratio;

        // Safety timeout: move stale active touches (no update for 2s) to fading
        let stale_ids: Vec<i8> = self
            .active_touches
            .iter()
            .filter(|(_, touch)| t - touch.last_update > 2.0)
            .map(|(&id, _)| id)
            .collect();
        for id in stale_ids {
            if let Some(mut touch) = self.active_touches.remove(&id) {
                touch.end_time = Some(t);
                self.fading_touches.push(touch);
            }
        }

        // Identity model matrix for overlay drawing
        const IDENTITY: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        // Push orthographic projection
        let proj_array: [f32; 16] = [
            1.0 / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ];
        let orig_proj = ctx.renderer.projection;
        ctx.renderer.set_projection(&proj_array);

        // Remove fully faded touches
        self.fading_touches.retain(|touch| {
            if let Some(end) = touch.end_time {
                if t > end + TOUCH_FADE_TIME {
                    return false;
                }
            }
            true
        });

        // Helper closure to draw a single touch
        let draw_touch = |renderer: &mut Renderer, touch: &mut ActiveTouch| {
            if t < touch.start_time {
                return;
            }
            let alpha = if let Some(end) = touch.end_time {
                if t < end {
                    TOUCH_ALPHA
                } else {
                    let fade_progress = ((t - end) / TOUCH_FADE_TIME).min(1.0);
                    TOUCH_ALPHA * (1.0 - fade_progress)
                }
            } else {
                TOUCH_ALPHA
            };
            if alpha <= 0.0 {
                return;
            }
            touch.anim.set_time(t);
            let pos = touch.anim.now();

            renderer.draw_circle(
                pos.x * aspect,
                -pos.y,
                TOUCH_RADIUS * 2.5,
                TOUCH_COLOR[0],
                TOUCH_COLOR[1],
                TOUCH_COLOR[2],
                alpha,
                &IDENTITY,
            );
        };

        // Draw active touches
        let ctx = self.render_ctx.as_mut().unwrap();
        for touch in self.active_touches.values_mut() {
            draw_touch(&mut ctx.renderer, touch);
        }
        // Draw fading touches
        for touch in &mut self.fading_touches {
            draw_touch(&mut ctx.renderer, touch);
        }

        ctx.renderer.set_projection(&orig_proj);
    }
}
