//! Phira Web Monitor - WASM Client
//!
//! This crate contains the WASM-specific logic, including:
//! - Data decoding (from proxy)
//! - WebGL rendering (TODO)
//! - Audio playback (TODO)

use monitor_common::core::{Chart, NoteKind};
use std::collections::HashSet;
use wasm_bindgen::prelude::*; // Prepare for hit tracking

pub mod particles;
pub mod renderer;
pub mod shaders;

// Initialize logging
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("error initializing logger");
}

#[wasm_bindgen]
pub struct WebGlRenderer {
    renderer: renderer::Renderer,
    chart: Option<Chart>,

    // Autoplay & Effects
    autoplay: bool,
    // Actually, we need a better way to ID notes.
    // For this demo, let's just track (line_index, note_index)
    processed_notes_indices: HashSet<(usize, usize)>,

    particle_system: particles::ParticleSystem,
}

#[wasm_bindgen]
impl WebGlRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<WebGlRenderer, JsValue> {
        let renderer = renderer::Renderer::new(canvas_id)?;
        Ok(Self {
            renderer,
            chart: None,
            autoplay: false,
            processed_notes_indices: HashSet::new(),
            particle_system: particles::ParticleSystem::new(),
        })
    }

    pub fn set_autoplay(&mut self, enabled: bool) {
        self.autoplay = enabled;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    pub fn load_chart(&mut self, data: &[u8]) -> Result<(), JsValue> {
        match bincode::deserialize::<Chart>(data) {
            Ok(chart) => {
                log::info!(
                    "Chart loaded into renderer: {} lines, {} notes",
                    chart.line_count(),
                    chart.note_count()
                );
                self.chart = Some(chart);
                Ok(())
            }
            Err(e) => Err(JsValue::from_str(&format!("Decode error: {}", e))),
        }
    }

    pub fn load_texture(&mut self, id: u32, width: u32, height: u32, data: &[u8]) {
        self.renderer.create_texture(id, width, height, data);
    }

    pub fn render(&mut self, time: f64) {
        // === PHASE 1: SETUP ===
        self.renderer.render(time);

        let screen_w = self.renderer.width as f32;
        let screen_h = self.renderer.height as f32;
        let dt = 0.016;

        // Phira's world coordinate system (from prpr/src/core/resource.rs L436, L448):
        // - X: -1.0 to 1.0 (left to right)
        // - Y: -1/aspect to 1/aspect (bottom to top)
        // - aspect_ratio = width / height (NOT height/width!)
        // - Camera zoom: vec2(1., -aspect_ratio)
        let aspect = screen_w / screen_h; // Phira: aspect_ratio = width/height

        // Scale factor: world unit of 1.0 = half screen width
        let world_scale = screen_w / 2.0;

        // Screen center
        let center_x = screen_w / 2.0;
        let center_y = screen_h / 2.0;

        // Line length in world units (from prpr/src/info.rs L78)
        const LINE_LENGTH: f32 = 6.0;

        // Note width in world units (from prpr/src/core.rs L15)
        const NOTE_WIDTH_RATIO_BASE: f32 = 0.13175016;

        self.particle_system.update(dt);

        if let Some(chart) = &mut self.chart {
            let chart_time = time as f32 - chart.offset;
            chart.set_time(chart_time);

            // === PHASE 2: RENDER LINES AND NOTES ===
            for (line_idx, line) in chart.lines.iter().enumerate() {
                let line_alpha = line.object.now_alpha();
                if line_alpha <= 0.0 {
                    continue;
                }

                // Line transform (normalized coordinates)
                let trans = line.object.now_translation(aspect);
                let (lx, ly) = (trans.x, trans.y);
                let l_rot = line.object.rotation.now().to_radians();

                // Convert to screen coordinates
                // Phira: tr.y /= res.aspect_ratio (from prpr/src/core/object.rs L55)
                let screen_lx = center_x + lx * world_scale;
                let screen_ly = center_y - (ly / aspect) * world_scale;

                // Draw judgment line
                self.renderer.use_texture(999);
                let line_w = LINE_LENGTH * 2.0 * world_scale;
                let line_h = 0.01 * world_scale; // ~5px at 1000px screen
                self.renderer.draw_rotated_rect(
                    screen_lx, screen_ly, line_w, line_h, -l_rot, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0,
                    1.0, line_alpha,
                );

                // Precompute rotation
                let (sin_rot, cos_rot) = l_rot.sin_cos();
                let line_height = line.now_height();

                // Fadeout time constant (from prpr/src/core/note.rs L9)
                const FADEOUT_TIME: f32 = 0.16;

                // === PHASE 3: RENDER NOTES ===
                for (note_idx, note) in line.notes.iter().enumerate() {
                    let key = (line_idx, note_idx);
                    let is_hold = matches!(note.kind, NoteKind::Hold { .. });

                    // Autoplay: trigger hit effect (only for non-fake notes)
                    if !note.fake
                        && self.autoplay
                        && !self.processed_notes_indices.contains(&key)
                        && chart_time >= note.time
                    {
                        self.processed_notes_indices.insert(key);
                        self.particle_system
                            .spawn(screen_lx, screen_ly, (1.0, 0.9, 0.5));
                    }

                    // === VISIBILITY LOGIC (Phira L228-232) ===
                    // In autoplay mode: judged non-hold notes disappear immediately
                    if self.autoplay && !note.fake && chart_time >= note.time && !is_hold {
                        continue;
                    }

                    // Non-autoplay (miss mode): skip non-hold notes after FADEOUT_TIME
                    if !self.autoplay
                        && !note.fake
                        && chart_time - FADEOUT_TIME >= note.time
                        && !is_hold
                    {
                        continue;
                    }

                    // Note translation (needed for CtrlObject calculation and rendering)
                    let trans = note.object.now_translation(aspect);
                    let (tx, ty) = (trans.x, trans.y);

                    // === CTRL OBJECT LOGIC ===
                    // Calculate distance from judge line ("height" in Phira terms)
                    // Formula (Phira L166): (height - line_height + ty / speed) * RPE_HEIGHT / 2.0
                    // RPE_HEIGHT = 900.0, so scale is 450.0
                    let dist = (note.height - line_height + ty / note.speed) * 450.0;
                    let mut ctrl_obj = line.ctrl_obj.clone(); // Clone to mutate state locally
                    ctrl_obj.set_height(dist);

                    // 1. Alpha Control
                    let ctrl_alpha = ctrl_obj.alpha.now();
                    let mut note_alpha = note.object.now_alpha() * ctrl_alpha;

                    if note_alpha <= 0.0 {
                        continue;
                    }

                    // Fadeout only applies to missed notes (non-autoplay, non-hold)
                    // In autoplay, notes are already skipped above when judged
                    // Hold notes never fade, they just shrink
                    if !self.autoplay && !is_hold && chart_time > note.time {
                        let fadeout_factor =
                            ((note.time - chart_time).min(0.0) / FADEOUT_TIME + 1.0).max(0.0);
                        note_alpha *= fadeout_factor;
                    }

                    // 2. Speed/Y Control (affects vertical position/speed)
                    let ctrl_y = ctrl_obj.y.now(); // Acts as speed multiplier in Phira
                    let spd = note.speed * ctrl_y;

                    // Note position calculation (Phira formula)
                    // line_height and height are divided by aspect_ratio, then multiplied by speed
                    let scaled_line_height = line_height / aspect * spd;
                    let scaled_note_height = note.height / aspect * spd;
                    let base = scaled_note_height - scaled_line_height;

                    // 3. Pos Control (X offset)
                    let ctrl_pos = ctrl_obj.pos.now();

                    // Note translation (adds to base)
                    // (tx, ty) already calculated above for CtrlObject logic

                    // X position: Phira uses multiplicative logic!
                    // L172: tr.x *= incline_val * ctrl_obj.pos.now_opt().unwrap_or(1.);
                    let local_x = tx * ctrl_pos;

                    // Notes below line (above=false) have inverted Y position
                    let local_y = if note.above {
                        base + ty
                    } else {
                        -(base + ty) // Invert for below-line notes
                    };

                    // 4. Size Control
                    let ctrl_size = ctrl_obj.size.now();
                    // This affects note width/height later during rendering

                    // Rotate by line rotation
                    let rotated_x = local_x * cos_rot - local_y * sin_rot;
                    let rotated_y = local_x * sin_rot + local_y * cos_rot;

                    // Convert to screen (Y inverted)
                    let note_x = screen_lx + rotated_x * world_scale;
                    let note_y = screen_ly - rotated_y * world_scale;

                    // Culling (rough bounds check)
                    if note_y < -200.0 || note_y > screen_h + 500.0 {
                        continue;
                    }

                    // Select texture
                    let tex_id = match note.kind {
                        NoteKind::Click => 1,
                        NoteKind::Drag => 2,
                        NoteKind::Flick => 3,
                        NoteKind::Hold { .. } => 4,
                    };
                    self.renderer.use_texture(tex_id);

                    // Note dimensions (from prpr/src/core/note.rs L110-111: draw_center)
                    // Width = scale * 2, Height = scale * 2 * (tex_h / tex_w)
                    // Apply ctrl_size here as scale multiplier
                    let scale_factor = world_scale * ctrl_size;
                    let note_w = NOTE_WIDTH_RATIO_BASE * 2.0 * scale_factor;
                    let note_h =
                        if let Some((tex_w, tex_h)) = self.renderer.get_texture_size(tex_id) {
                            note_w * (tex_h as f32 / tex_w as f32)
                        } else {
                            note_w / 10.5 // Fallback: typical click.png ratio
                        };

                    // === HOLD NOTE RENDERING ===
                    if let NoteKind::Hold {
                        end_time,
                        end_height,
                    } = note.kind
                    {
                        // Skip if hold is complete
                        if chart_time >= end_time {
                            continue;
                        }

                        let scaled_end_height = end_height / aspect * spd;

                        // Phira logic (from prpr/src/core/note.rs L266-270):
                        // h = line_height if being held (time <= now), else note.height
                        // bottom = h - line_height
                        // top = end_height - line_height
                        let h = if note.time <= chart_time {
                            scaled_line_height
                        } else {
                            scaled_note_height
                        };
                        let bottom = h - scaled_line_height + ty;
                        let top = scaled_end_height - scaled_line_height + ty;

                        let body_len = top - bottom;
                        if body_len <= 0.0 {
                            continue;
                        }

                        // Hold texture dimensions for head/tail calculation
                        // Phira: head_h = r.h/r.w * scale * hold_ratio
                        // where hold_ratio = tex_h/tex_w, r = UV rect
                        // For default atlas (50, 50) in 200x1920 texture:
                        // - Head UV: (0, 1-50/1920, 1, 50/1920) -> r.h=0.026, r.w=1.0
                        // - Tail UV: (0, 0, 1, 50/1920) -> r.h=0.026, r.w=1.0
                        // head_h = 0.026/1.0 * scale * (1920/200) = 0.026 * 9.6 * scale = 0.25 * scale
                        let (hold_tex_w, hold_tex_h) = self
                            .renderer
                            .get_texture_size(tex_id)
                            .unwrap_or((200, 1920));

                        // Default atlas: tail=50px, head=50px from 1920px height
                        let atlas_tail = 50.0;
                        let atlas_head = 50.0;
                        let head_uv_h = atlas_head / hold_tex_h as f32;
                        let tail_uv_h = atlas_tail / hold_tex_h as f32;

                        // Head/tail height in world units (Phira formula: r.h/r.w * scale * ratio)
                        // r.w = 1.0 (full width), r.h = atlas_px/tex_h
                        // Result: (atlas/tex_h) * note_w/2 * (tex_h/tex_w) = atlas/tex_w * note_w/2
                        let head_h = (atlas_head / hold_tex_w as f32) * note_w;
                        let tail_h = (atlas_tail / hold_tex_w as f32) * note_w;

                        let mid_y = (bottom + top) / 2.0;

                        // Rotate and draw body
                        let body_rx = local_x * cos_rot - mid_y * sin_rot;
                        let body_ry = local_x * sin_rot + mid_y * cos_rot;
                        let body_sx = screen_lx + body_rx * world_scale;
                        let body_sy = screen_ly - body_ry * world_scale;

                        // Body UVs: (0, tail_uv, 1, 1-head_uv-tail_uv)
                        let body_uv_start = tail_uv_h;
                        let body_uv_end = 1.0 - head_uv_h;

                        self.renderer.draw_rotated_rect(
                            body_sx,
                            body_sy,
                            note_w,
                            body_len * world_scale,
                            -l_rot,
                            0.0,
                            body_uv_start,
                            1.0,
                            body_uv_end,
                            1.0,
                            1.0,
                            1.0,
                            note_alpha * 0.8,
                        );

                        // Draw head ONLY if not being held yet
                        if chart_time < note.time {
                            // Head position: at bottom, offset down by head_h/2 for centered drawing
                            let head_y = bottom;
                            let head_rx = local_x * cos_rot - head_y * sin_rot;
                            let head_ry = local_x * sin_rot + head_y * cos_rot;

                            // Head UVs: (0, 1-head_uv, 1, head_uv)
                            self.renderer.draw_rotated_rect(
                                screen_lx + head_rx * world_scale,
                                screen_ly - head_ry * world_scale,
                                note_w,
                                head_h,
                                -l_rot,
                                0.0,
                                1.0 - head_uv_h,
                                1.0,
                                1.0,
                                1.0,
                                1.0,
                                1.0,
                                note_alpha,
                            );
                        }

                        // Draw tail (always at top)
                        let tail_y = top;
                        let tail_rx = local_x * cos_rot - tail_y * sin_rot;
                        let tail_ry = local_x * sin_rot + tail_y * cos_rot;

                        // Tail UVs: (0, 0, 1, tail_uv)
                        self.renderer.draw_rotated_rect(
                            screen_lx + tail_rx * world_scale,
                            screen_ly - tail_ry * world_scale,
                            note_w,
                            tail_h,
                            -l_rot,
                            0.0,
                            0.0,
                            1.0,
                            tail_uv_h,
                            1.0,
                            1.0,
                            1.0,
                            note_alpha,
                        );
                    } else {
                        // === SIMPLE NOTE (Click/Drag/Flick) ===
                        self.renderer.draw_rotated_rect(
                            note_x, note_y, note_w, note_h, -l_rot, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0,
                            1.0, note_alpha,
                        );
                    }
                }
            }
        }

        // === PHASE 4: PARTICLES ===
        // Particles are updated above, drawn by renderer's flush
    }
}

/// A simple test function to verify WASM is working.
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Phira Web Monitor is ready.", name)
}

/// Decode a bincode-encoded chart from server
#[wasm_bindgen]
pub fn decode_chart(data: &[u8]) -> Result<JsValue, JsValue> {
    log::info!("Decoding {} bytes of chart data", data.len());
    match bincode::deserialize::<Chart>(data) {
        Ok(chart) => chart_to_json(&chart),
        Err(e) => Err(JsValue::from_str(&format!("Decode error: {}", e))),
    }
}

fn chart_to_json(chart: &Chart) -> Result<JsValue, JsValue> {
    let info = serde_json::json!({
        "success": true,
        "offset": chart.offset,
        "lineCount": chart.line_count(),
        "noteCount": chart.note_count(),
    });
    Ok(JsValue::from_str(&info.to_string()))
}
