use core::f32;

use crate::engine::note::{RenderConfig, draw_note};
use crate::engine::resource::Resource;
use crate::renderer::Renderer;
use monitor_common::core::{ChartSettings, JudgeLine, JudgeLineKind};
use nalgebra::{Matrix3, Rotation2, Vector2};

pub fn draw_line(
    res: &mut Resource,
    line: &JudgeLine,
    renderer: &mut Renderer,
    line_index: usize,
    settings: &ChartSettings,
) {
    let translation = line.object.now_translation(res.aspect_ratio);
    // Convert Vector to Vector2 (MonitorCommon Vector is Vector2<f32>)
    let translation = Vector2::new(translation.x, translation.y);
    let rot = line.object.rotation.now_opt().unwrap_or(0.0);
    let rotation = Rotation2::new(rot.to_radians());

    let mut transform = Matrix3::identity();
    transform
        .fixed_view_mut::<2, 2>(0, 0)
        .copy_from(rotation.matrix());
    transform[(0, 2)] = translation.x;
    transform[(1, 2)] = translation.y;

    res.with_model(transform, |res| {
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
                let len = 6.0;
                let thickness = 0.01;

                renderer.set_texture(&renderer.white_texture.clone());
                renderer.draw_rect(
                    -len / 2.0,
                    -thickness / 2.0,
                    len,
                    thickness,
                    color.r,
                    color.g,
                    color.b,
                    alpha * color.a,
                    &res.get_gl_matrix(),
                );
            }
            JudgeLineKind::Texture(_, _) => {
                if let Some(texture) = res.line_textures.get(&line_index) {
                    let scale_x = line.object.scale.x.now_opt().unwrap_or(1.0);
                    let scale_y = line.object.scale.y.now_opt().unwrap_or(1.0);

                    let w = scale_x;
                    let h = scale_y * (texture.height as f32 / texture.width as f32);

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
            _ => {}
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
        for note in &line.notes {
            draw_note(res, note, line, &config, renderer);
        }
    });
}
