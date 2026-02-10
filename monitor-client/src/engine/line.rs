use core::f32;

use crate::engine::note::{RenderConfig, draw_note};
use crate::engine::resource::{Resource, Vector};
use crate::renderer::Renderer;
use monitor_common::core::{ChartSettings, JudgeLine, JudgeLineKind, Matrix};

pub fn draw_line(
    res: &mut Resource,
    line: &JudgeLine,
    length: f32,
    renderer: &mut Renderer,
    line_index: usize,
    settings: &ChartSettings,
    world_matrix: Matrix,
) {
    // TODO: support attach_ui
    if let Some(_) = &line.attach_ui {
        return;
    }
    res.with_model(world_matrix, |res| {
        let alpha = line.object.alpha.now_opt().unwrap_or(1.0);

        // PE Alpha Extension Logic (Negative Alpha)
        let mut draw_below = line.show_below;
        let mut _appear_before = f32::INFINITY;

        if alpha < 0.0 {
            if !settings.pe_alpha_extension {
                return;
            }
            let w = (-alpha).floor() as u32;
            match w {
                1 => {
                    return;
                }
                2 => {
                    draw_below = false;
                }
                w if (100..1000).contains(&w) => {
                    _appear_before = (w as f32 - 100.) / 10.;
                }
                _ => {}
            }
        }

        let color = line.color.now_opt().unwrap_or(monitor_common::core::Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });

        match &line.kind {
            JudgeLineKind::Normal => {
                let thickness = 0.01;

                renderer.set_texture(&renderer.white_texture.clone());
                renderer.draw_rect(
                    -length / 2.0,
                    -thickness / 2.0,
                    length,
                    thickness,
                    color.r,
                    color.g,
                    color.b,
                    alpha * color.a,
                    &res.get_gl_matrix(),
                );
                if f32::abs(alpha * color.a) > 0.001 {
                    web_sys::console::log_1(
                        &format!(
                            "DEBUG Line {}: alpha={:.3}, color=({:.2},{:.2},{:.2},{:.2})",
                            line_index, alpha, color.r, color.g, color.b, color.a
                        )
                        .into(),
                    );
                }
            }
            JudgeLineKind::Texture(_, _) => {
                if let Some(texture) = res.line_textures.get(&line_index) {
                    let scale_x = line.object.scale.x.now_opt().unwrap_or(1.0);
                    let scale_y = line.object.scale.y.now_opt().unwrap_or(1.0);

                    // Note: RPE scale (2/1350) is already included in the animation scale from the proxy
                    let w = scale_x * (texture.width as f32);
                    let h = scale_y * (texture.height as f32);

                    renderer.set_texture(texture);
                    renderer.draw_texture_rect(
                        -w / 2.0,
                        -h / 2.0,
                        w,
                        h,
                        0.0,
                        0.0,
                        1.0,
                        1.0,
                        color.r,
                        color.g,
                        color.b,
                        alpha * color.a,
                        &res.get_gl_matrix(),
                    );
                }
            }
            JudgeLineKind::TextureGif(_, gif, _) => {
                if let Some(frames) = res.line_gif_textures.get(&line_index) {
                    let time = res.time * 1000.0; // convert to ms
                    let total_time = gif.total_time as f32;
                    let current_time = if total_time > 0.0 {
                        time % total_time
                    } else {
                        0.0
                    };

                    let mut frame_index = 0;
                    for (i, (frame_time, _)) in gif.frames.iter().enumerate() {
                        if (*frame_time as f32) > current_time {
                            break;
                        }
                        frame_index = i;
                    }

                    if let Some(texture) = frames.get(frame_index) {
                        let scale_x = line.object.scale.x.now_opt().unwrap_or(1.0);
                        let scale_y = line.object.scale.y.now_opt().unwrap_or(1.0);

                        // Note: RPE scale (2/1350) is already included in the animation scale from the proxy
                        let w = scale_x * (texture.width as f32);
                        let h = scale_y * (texture.height as f32);

                        renderer.set_texture(texture);
                        renderer.draw_texture_rect(
                            -w / 2.0,
                            -h / 2.0,
                            w,
                            h,
                            0.0,
                            0.0,
                            1.0,
                            1.0,
                            color.r,
                            color.g,
                            color.b,
                            alpha * color.a,
                            &res.get_gl_matrix(),
                        );
                    }
                }
            }
            JudgeLineKind::Text(anim) => {
                if let Some(font) = &res.font {
                    let text = anim.now_opt().unwrap_or_default();
                    let rpe_scale = 2.0 / 1350.0;

                    font.draw_text(
                        renderer,
                        &text,
                        0.0,
                        0.0,
                        60.0 * rpe_scale,
                        0.5,
                        &res.get_gl_matrix(),
                    );
                }
            }
            JudgeLineKind::Paint(_) => {
                // TODO: Implement Paint rendering
            }
        }

        let height_val = line.height.now_opt().unwrap_or(0.0);

        let config = RenderConfig {
            line_height: height_val,
            aspect_ratio: res.aspect_ratio,
            note_width: res.note_width * res.note_scale,
            draw_below: draw_below,
            alpha: line.ctrl_obj.alpha.now_opt().unwrap_or(1.0),
        };

        // Draw notes
        // Pass 1: Above notes
        for note in line.notes.iter().filter(|n| n.above) {
            draw_note(res, note, line, &config, renderer);
        }

        // Pass 2: Below notes (mirrored Y)
        res.with_model(
            Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)),
            |res| {
                for note in line.notes.iter().filter(|n| !n.above) {
                    draw_note(res, note, line, &config, renderer);
                }
            },
        );
    });
}
