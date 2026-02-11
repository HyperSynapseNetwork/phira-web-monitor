use crate::engine::resource::Resource;
use crate::renderer::{Renderer, Texture};
use monitor_common::core::{JudgeLine, JudgeStatus, Note, NoteKind};
use nalgebra::{Matrix3, Vector2};

pub struct RenderConfig {
    pub line_height: f32,
    pub aspect_ratio: f32,
    pub note_width: f32,
    pub draw_below: bool,
    pub alpha: f32,
}

pub fn draw_note(
    res: &mut Resource,
    note: &Note,
    _line: &JudgeLine,
    config: &RenderConfig,
    renderer: &mut Renderer,
) {
    // Gate rendering by judge status
    match &note.judge {
        JudgeStatus::Judged => {
            if !matches!(note.kind, NoteKind::Hold { .. }) {
                // Click/Drag/Flick: stop rendering once judged
                return;
            }
            // Hold notes that are Judged = miss; will render at 50% alpha below
        }
        _ => {}
    }

    let res_pack = res.res_pack.as_ref().unwrap();
    let style_ref = if note.multiple_hint {
        &res_pack.note_style_mh
    } else {
        &res_pack.note_style
    };

    // Phira's double_hint scaling: multi-hint notes are wider by the ratio
    // of mh texture width to normal texture width (prpr note.rs L199-203)
    let scale = if note.multiple_hint {
        let ratio =
            res_pack.note_style_mh.click.width as f32 / res_pack.note_style.click.width as f32;
        config.note_width * ratio
    } else {
        config.note_width
    };

    // Alpha modifier: Judged hold notes (miss) render at 50%
    let judge_alpha = if matches!(note.judge, JudgeStatus::Judged)
        && matches!(note.kind, NoteKind::Hold { .. })
    {
        0.5
    } else {
        1.0
    };

    match &note.kind {
        NoteKind::Click => {
            draw_simple_note(
                res,
                note,
                style_ref.click.clone(),
                scale,
                config,
                renderer,
                judge_alpha,
            );
        }
        NoteKind::Drag => {
            draw_simple_note(
                res,
                note,
                style_ref.drag.clone(),
                scale,
                config,
                renderer,
                judge_alpha,
            );
        }
        NoteKind::Flick => {
            draw_simple_note(
                res,
                note,
                style_ref.flick.clone(),
                scale,
                config,
                renderer,
                judge_alpha,
            );
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
                scale,
                config,
                renderer,
                *end_time,
                *end_height,
                judge_alpha,
            );
        }
    }
}

fn draw_simple_note(
    res: &mut Resource,
    note: &Note,
    texture: Texture,
    scale: f32,
    config: &RenderConfig,
    renderer: &mut Renderer,
    judge_alpha: f32,
) {
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
        let alpha = note.object.alpha.now_opt().unwrap_or(1.0) * config.alpha * judge_alpha;

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
    scale: f32,
    config: &RenderConfig,
    renderer: &mut Renderer,
    _end_time: f32,
    end_height: f32,
    judge_alpha: f32,
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

    // For active Hold notes, clamp head to line position (head doesn't go below line)
    let clamped_head_y = if matches!(note.judge, JudgeStatus::Hold(..)) {
        raw_head_y.max(0.0)
    } else {
        raw_head_y
    };

    let x = note.object.translation.x.now_opt().unwrap_or(0.0);
    let transform = Matrix3::new_translation(&Vector2::new(x, 0.0));
    res.with_model(transform, |res| {
        let obj_scale_x = note.object.scale.x.now_opt().unwrap_or(1.0);
        let width = scale * 2.0 * obj_scale_x;
        let alpha = note.object.alpha.now_opt().unwrap_or(1.0) * config.alpha * judge_alpha;

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
        let tex_aspect = texture.height as f32 / texture.width as f32;

        let head_h = width * (head_rect.h / head_rect.w) * tex_aspect;
        let tail_h = width * (tail_rect.h / tail_rect.w) * tex_aspect;

        // Use clamped head position for active holds
        let head_y = clamped_head_y;
        let tail_y = raw_tail_y;

        let is_compact = res.res_pack.as_ref().map_or(false, |p| p.info.hold_compact);

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
