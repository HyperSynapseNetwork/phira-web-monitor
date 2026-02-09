use crate::engine::resource::Resource;
use crate::renderer::{Renderer, Texture};
use monitor_common::core::{JudgeLine, Note, NoteKind};
use nalgebra::{Matrix3, Vector2};

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
    let (click, drag, flick, _hold) = {
        let style = &res.res_pack.as_ref().unwrap().note_style;
        (
            style.click.clone(),
            style.drag.clone(),
            style.flick.clone(),
            style.hold.clone(),
        )
    };

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

            // For Hold notes, the y-position of the effect should be at the judge line (0.0).
            // The note itself moves, but the "hit" happens at the judge line.
            // Our coordinate system has judge line at y=0?
            // "y_pos = (note_height_val - line_height_val) ..."
            // When note is at judge line, note_height == line_height => y_pos = 0.
            // So emitting at origin (0,0) of the note's transform is correct IF the note is at the line.
            // But for a Hold note, the "Head" passes the line, then the Body passes.
            // The effect should stay at the Judge Line.
            //
            // Current `res.emit_at_origin` uses the MODEL matrix.
            // If we use `with_model(transform)`, we are applying the note's current position.
            // If the note is falling, its y_pos is changing.
            //
            // For Click/Drag/Flick, they are emitted exactly when y_pos ~ 0.
            // For Hold, if we emit at `note.object` position, the particles will spawn where the head is (which is falling below line).
            //
            // We need to emit at the Judge Line position relative to the note?
            // Or easier: Emit at World Coordinates (x, 0).
            //
            // Let's look at Phira:
            // "res.emit_at_origin(line.notes[id as usize].rotation(&line), ...)"
            // It uses `line.now_transform(...) * note_transform`.
            // In Phira, `note.object.set_time(t)` is called.
            // For Hold, `t` is the current time.
            // If `note.object` tracks the note's position at time `t`, and for a Hold note the "active part" is at the judge line...
            // Actually, in Phira, the note object might define the abstraction.
            //
            // In Monitor:
            // `draw_simple_note` calculates `y_pos`.
            // The `transform` passed to `with_model` uses this `y_pos`.
            // If we use the same transform logic for Hold particles, they will spawn at `y_pos`.
            // `y_pos` for a Hold note head at `time > note.time` will be NEGATIVE (below line).
            // We want particles at Y=0 (Judge Line).
            //
            // Solution: Use a transform that places the emitter at Y=0.
            // The X position is determined by the note's X (which might be moving if it's a moving trace, but usually static relative to line).
            // Let's assume X is `note.object.translation.x`.
            //
            // So we should NOT use the note's falling Y position for the effect.
            // We should use Y=0.

            let transform = Matrix3::new_translation(&Vector2::new(x, 0.0));

            // Note: `draw_simple_note` uses `y_pos` in its transform translation.
            // Here `x` is just x-offset. `0.0` means Y=0.
            // So `with_model` will place us at (X, 0).
            // This is correct for the Judge Line.

            res.with_model(transform, |res| {
                if let Some(info) = res.res_pack.as_ref().map(|p| &p.info) {
                    let color = info.fx_perfect();
                    res.emit_at_origin(0.0, color);
                }
            });
        }
    }

    match &note.kind {
        NoteKind::Click => {
            draw_simple_note(res, note, click, config, renderer);
        }
        NoteKind::Drag => {
            draw_simple_note(res, note, drag, config, renderer);
        }
        NoteKind::Flick => {
            draw_simple_note(res, note, flick, config, renderer);
        }
        NoteKind::Hold {
            end_time,
            end_height,
        } => {
            let style_ref = &res.res_pack.as_ref().unwrap().note_style;
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

                // Adjust UV
                // r.y is bottom V? Wait, coordinate system for UV?
                // Usually V=0 is bottom, V=1 is top? Or reversed?
                // In OpenGL/MiniQuad: (0,0) is usually bottom-left?
                // But images are often loaded top-down.
                // Let's check texture loading. `image` crate loads top-left.
                // NoteStyle rects:
                // hold_head_rect: new(0., 1.-sy, 1., sy).  (Top of image)
                // hold_tail_rect: new(0., 0., 1., ey). (Bottom of image)
                // So V=0 is Bottom of image?
                // If V=0 is Bottom, then:
                // Head (Top of image) has high V.
                // Tail (Bottom of image) has low V.
                //
                // But "Head" is start of note (Bottom visually in falling).
                // "Tail" is end of note (Top visually).
                // So Head Part uses Head Rect (Top of Image).
                // Tail Part uses Tail Rect (Bottom of Image).
                //
                // draw_texture_rect args: u, v, us, vs.
                // If I clip the bottom of the rendered quad (which corresponds to bottom of the texture part),
                // I should increase V if V increases upwards.
                //
                // Let's assume V increases upwards (0=Bottom, 1=Top).
                // draw_v = r.y + (diff / h) * r.h;
                // draw_vs = r.h * (draw_h / h);

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
