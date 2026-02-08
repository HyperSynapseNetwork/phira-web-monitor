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

    // Use (line - note) to correct direction (note coming from top)
    // Positive result = Below (Passed), Negative result = Above (Future)
    let y_pos = (line_height_val - note_height_val) * spd / config.aspect_ratio;

    // If y_pos > 0, it means it's below the line.
    // If not drawing below, skip.
    if !config.draw_below && y_pos > 0.001 {
        return;
    }

    let transform = Matrix3::new_translation(&Vector2::new(x, y_pos));
    res.with_model(transform, |res| {
        let obj_scale_x = note.object.scale.x.now_opt().unwrap_or(1.0);

        let w = scale * obj_scale_x;
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
    _head_rect: crate::engine::resource::Rect,
    _body_rect: crate::engine::resource::Rect,
    _tail_rect: crate::engine::resource::Rect,
    config: &RenderConfig,
    renderer: &mut Renderer,
    _end_time: f32,
    end_height: f32,
) {
    let spd = note.speed;
    let line_height_val = config.line_height;

    let note_height_val = note.height;
    let note_end_height_val = end_height;

    // Calculate Y positions relative to Judge Line (0.0)
    // Head Y: Start of hold. (line - start)
    // Tail Y: End of hold. (line - end)
    let raw_head_y = (line_height_val - note_height_val) * spd / config.aspect_ratio;
    let raw_tail_y = (line_height_val - note_end_height_val) * spd / config.aspect_ratio;

    // Clamp head to judge line (0.0) effectively "consuming" the hold as it plays
    let vis_head_y = raw_head_y.min(0.0);
    let vis_tail_y = raw_tail_y;

    // If tail is past the line (tail > 0), the note is fully finished.
    // Allow a small margin or just check strictly.
    if vis_tail_y > 0.0 {
        return;
    }

    // Determine rect dimensions
    // Draw from vis_tail_y (Top) to vis_head_y (Bottom)
    let rect_y = vis_tail_y;
    let rect_h = vis_head_y - vis_tail_y;

    if rect_h <= 0.0 {
        return;
    }

    let x = note.object.translation.x.now_opt().unwrap_or(0.0);
    let transform = Matrix3::new_translation(&Vector2::new(x, 0.0));
    res.with_model(transform, |res| {
        let scale = config.note_width;
        let obj_scale_x = note.object.scale.x.now_opt().unwrap_or(1.0);
        let width = scale * obj_scale_x;
        let alpha = note.object.alpha.now_opt().unwrap_or(1.0) * config.alpha;

        renderer.set_texture(&texture);

        // Draw Body covering the full length
        renderer.draw_texture_rect(
            -width / 2.0,
            rect_y,
            width,
            rect_h,
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
