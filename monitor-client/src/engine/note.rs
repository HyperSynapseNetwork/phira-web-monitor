use crate::engine::resource::Resource;
use crate::renderer::{Renderer, Texture};
use monitor_common::core::{JudgeLine, Note, NoteKind};
use nalgebra::{Matrix3, Vector2};
use std::f32::consts::PI;

pub struct RenderConfig {
    pub line_height: f32,
    pub aspect_ratio: f32,
    pub note_width: f32,
    pub draw_below: bool,
    pub alpha: f32,
}

const HOLD_PARTICLE_INTERVAL: f32 = 0.15;

pub fn draw_note(
    res: &mut Resource,
    note: &Note,
    _line: &JudgeLine,
    config: &RenderConfig,
    renderer: &mut Renderer,
) {
    // Hit Effect Logic
    if !note.fake {
        // Monitor (Auto-Play) logic:
        // 1. Trigger once for Click/Drag/Flick when time crosses note.time
        // 2. Trigger at fixed frequency for Hold while time is within [note.time, end_time]

        let should_emit = match note.kind {
            NoteKind::Hold { end_time, .. } => {
                if res.dt > 0.0 && res.time >= note.time && res.time <= end_time {
                    let n_time = res.time - note.time;
                    let n_prev_time = res.time - res.dt - note.time;

                    let current_tick = (n_time / HOLD_PARTICLE_INTERVAL).floor() as i32;
                    let prev_tick = if n_prev_time < 0.0 {
                        -1
                    } else {
                        (n_prev_time / HOLD_PARTICLE_INTERVAL).floor() as i32
                    };

                    current_tick > prev_tick
                } else {
                    false
                }
            }
            _ => {
                // For others: emit only on the frame we cross note.time
                res.dt > 0.0 && note.time > res.time - res.dt && note.time <= res.time
            }
        };

        if should_emit {
            let x = note.object.translation.x.now_opt().unwrap_or(0.0);
            let transform = Matrix3::new_translation(&Vector2::new(x, 0.0));

            res.with_model(transform, |res| {
                if let Some(info) = res.res_pack.as_ref().map(|p| &p.info) {
                    let color = info.fx_perfect();
                    let rotation = if note.above { 0.0 } else { PI };
                    res.emit_at_origin(rotation, color);
                }
            });
        }
    }

    let style_ref = if note.multiple_hint {
        &res.res_pack.as_ref().unwrap().note_style_mh
    } else {
        &res.res_pack.as_ref().unwrap().note_style
    };
    match &note.kind {
        NoteKind::Click => {
            draw_simple_note(res, note, style_ref.click.clone(), config, renderer);
        }
        NoteKind::Drag => {
            draw_simple_note(res, note, style_ref.drag.clone(), config, renderer);
        }
        NoteKind::Flick => {
            draw_simple_note(res, note, style_ref.flick.clone(), config, renderer);
        }
        NoteKind::Hold {
            end_time,
            end_height,
        } => {
            let head_rect = style_ref.hold_head_rect();
            let body_rect = style_ref.hold_body_rect();
            let tail_rect = style_ref.hold_tail_rect();
            let hold_tex = style_ref.hold.clone();

            draw_hold_note(
                res,
                note,
                hold_tex,
                head_rect,
                body_rect,
                tail_rect,
                config,
                renderer,
                *end_time,
                *end_height,
            );
        }
    }
}

fn draw_simple_note(
    res: &mut Resource,
    note: &Note,
    texture: Texture,
    config: &RenderConfig,
    renderer: &mut Renderer,
) {
    let scale = config.note_width;
    let x = note.object.translation.x.now_opt().unwrap_or(0.0);

    let spd = note.speed;
    let line_height_val = config.line_height;
    let note_height_val = note.height;

    // Use (note - line) because coordinate system is Positive Up.
    // Future Note: note > line. Result Positive (Above).
    let y_pos = (note_height_val - line_height_val) * spd / config.aspect_ratio;

    // If y_pos < 0, it means it's below the line (Past).
    // If not drawing below, skip.
    if !config.draw_below && y_pos < -0.001 {
        return;
    }

    let transform = Matrix3::new_translation(&Vector2::new(x, y_pos));
    res.with_model(transform, |res| {
        let obj_scale_x = note.object.scale.x.now_opt().unwrap_or(1.0);

        let w = scale * 2.0 * obj_scale_x;
        // Adjust aspect ratio of texture
        let h = w * (texture.height as f32 / texture.width as f32);
        let alpha = note.object.alpha.now_opt().unwrap_or(1.0) * config.alpha;

        renderer.set_texture(&texture);
        renderer.draw_texture_rect(
            -w / 2.0,
            -h / 2.0,
            w,
            h,
            0.0,
            0.0,
            1.0,
            1.0,
            1.0,
            1.0,
            1.0,
            alpha,
            &res.get_gl_matrix(),
        );
    });
}

fn draw_hold_note(
    res: &mut Resource,
    note: &Note,
    texture: Texture,
    head_rect: crate::engine::resource::Rect,
    body_rect: crate::engine::resource::Rect,
    tail_rect: crate::engine::resource::Rect,
    config: &RenderConfig,
    renderer: &mut Renderer,
    _end_time: f32,
    end_height: f32,
) {
    let spd = note.speed;
    let line_height_val = config.line_height;

    let note_height_val = note.height;
    let note_end_height_val = end_height;

    let raw_head_y = (note_height_val - line_height_val) * spd / config.aspect_ratio;
    let raw_tail_y = (note_end_height_val - line_height_val) * spd / config.aspect_ratio;

    // If fully passed, return
    if raw_tail_y < 0.0 {
        return;
    }

    let x = note.object.translation.x.now_opt().unwrap_or(0.0);
    let transform = Matrix3::new_translation(&Vector2::new(x, 0.0));
    res.with_model(transform, |res| {
        let scale = config.note_width;
        let obj_scale_x = note.object.scale.x.now_opt().unwrap_or(1.0);
        let width = scale * 2.0 * obj_scale_x;
        let alpha = note.object.alpha.now_opt().unwrap_or(1.0) * config.alpha;

        renderer.set_texture(&texture);

        // Helper to draw a part with clipping at y=0
        // y: bottom position of the part
        // h: height of the part
        // r: source rect (u, v, w, h)
        let mut draw_part = |y: f32, h: f32, r: crate::engine::resource::Rect| {
            if h <= 0.0001 {
                return;
            }
            let mut draw_y = y;
            let mut draw_h = h;
            let mut draw_v = r.y;
            let mut draw_vs = r.h;

            // Clip bottom
            if draw_y < 0.0 {
                let diff = -draw_y;
                if diff >= draw_h {
                    return;
                } // Fully clipped
                draw_y = 0.0;
                draw_h -= diff;

                draw_v += (diff / h) * draw_vs;
                draw_vs *= draw_h / h;
            }

            renderer.draw_texture_rect(
                -width / 2.0,
                draw_y,
                width,
                draw_h,
                r.x,
                draw_v,
                r.w,
                draw_vs,
                1.0,
                1.0,
                1.0,
                alpha,
                &res.get_gl_matrix(),
            );
        };

        // Aspect ratio of texture parts
        // Isotropic scaling: no need for 1.5 correction.
        let tex_aspect = texture.height as f32 / texture.width as f32;

        let head_h = width * (head_rect.h / head_rect.w) * tex_aspect;
        let tail_h = width * (tail_rect.h / tail_rect.w) * tex_aspect;

        // Positions
        // Head is at start (raw_head_y).
        let head_y = raw_head_y;

        // Tail is at end (raw_tail_y).
        let tail_y = raw_tail_y;

        let is_compact = res.res_pack.as_ref().map_or(false, |p| p.info.hold_compact);

        // In Phira, head/tail are centered at the point (if compact) or slightly offset.
        // We'll align with prpr/src/core/note.rs logic:
        // Head drawn at bottom - (compact ? hf.y : hf.y * 2)
        // Since our draw_part(y, h) draws upwards from y:
        // centered at Y means y = Y - h/2.
        let draw_head_y = head_y - if is_compact { head_h / 2.0 } else { head_h };
        let draw_tail_y = tail_y - if is_compact { tail_h / 2.0 } else { 0.0 };

        // Body is between Head end and Tail start.
        let body_y = draw_head_y + head_h;
        let body_h = draw_tail_y - body_y;

        // Draw parts
        draw_part(draw_head_y, head_h, head_rect);
        // Ensure body has positive height
        if body_h > 0.01 {
            draw_part(body_y, body_h, body_rect);
        }
        draw_part(draw_tail_y, tail_h, tail_rect);
    });
}
