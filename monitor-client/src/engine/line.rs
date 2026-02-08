use crate::engine::note::{RenderConfig, draw_note};
use crate::engine::resource::Resource;
use crate::renderer::Renderer;
use monitor_common::core::{JudgeLine, JudgeLineKind};
use nalgebra::{Matrix3, Rotation2, Vector2};
use web_sys::console::debug;

pub fn draw_line(res: &mut Resource, line: &JudgeLine, renderer: &mut Renderer) {
    let trans = line.object.translation.x.now_opt().unwrap_or(0.0);
    let trans_y = line.object.translation.y.now_opt().unwrap_or(0.0);
    let rot = line.object.rotation.now_opt().unwrap_or(0.0);

    let rotation = Rotation2::new(rot.to_radians());
    let translation = Vector2::new(trans, trans_y);

    let mut transform = Matrix3::identity();
    transform
        .fixed_view_mut::<2, 2>(0, 0)
        .copy_from(rotation.matrix());
    transform[(0, 2)] = translation.x;
    transform[(1, 2)] = translation.y;

    res.with_model(transform, |res| {
        let alpha = line.object.alpha.now_opt().unwrap_or(1.0);

        match &line.kind {
            JudgeLineKind::Normal => {
                let len = 6.0;
                let thickness = 0.01;
                let color = line.color.now_opt().unwrap_or(monitor_common::core::Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                });

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
            JudgeLineKind::Texture(_tex, _) => {
                // Texture handling needs Texture resource from ResPack?
            }
            _ => {}
        }

        let height_val = line.height.now_opt().unwrap_or(0.0);

        let config = RenderConfig {
            line_height: height_val,
            aspect_ratio: res.aspect_ratio,
            note_width: res.note_width,
            draw_below: line.show_below,
            alpha: alpha,
        };

        // Draw notes
        for note in &line.notes {
            draw_note(res, note, line, &config, renderer);
        }
    });
}
