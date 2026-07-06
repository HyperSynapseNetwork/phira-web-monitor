#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Viewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewportLayout {
    pub viewport: Viewport,
    pub aspect_ratio: f32,
}

pub(crate) fn letterbox_viewport(
    width: u32,
    height: u32,
    design_ratio: f32,
) -> Option<ViewportLayout> {
    if width == 0 || height == 0 || design_ratio <= 0.0 {
        return None;
    }

    let screen_ratio = width as f32 / height as f32;
    let aspect_ratio = design_ratio.min(screen_ratio);

    let (vp_w, vp_h) = if screen_ratio > aspect_ratio {
        ((height as f32 * aspect_ratio).round() as u32, height)
    } else {
        (width, (width as f32 / aspect_ratio).round() as u32)
    };

    Some(ViewportLayout {
        viewport: Viewport {
            x: ((width - vp_w) / 2) as i32,
            y: ((height - vp_h) / 2) as i32,
            width: vp_w,
            height: vp_h,
        },
        aspect_ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letterbox_viewport_caps_wide_screens_to_design_ratio() {
        let layout = letterbox_viewport(1920, 1080, 4.0 / 3.0).unwrap();

        assert_eq!(
            layout.viewport,
            Viewport {
                x: 240,
                y: 0,
                width: 1440,
                height: 1080,
            }
        );
        assert!((layout.aspect_ratio - 4.0 / 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn letterbox_viewport_uses_full_width_on_narrow_screens() {
        let layout = letterbox_viewport(900, 1200, 16.0 / 9.0).unwrap();

        assert_eq!(
            layout.viewport,
            Viewport {
                x: 0,
                y: 0,
                width: 900,
                height: 1200,
            }
        );
        assert!((layout.aspect_ratio - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn letterbox_viewport_rejects_empty_dimensions() {
        assert_eq!(letterbox_viewport(0, 1080, 16.0 / 9.0), None);
        assert_eq!(letterbox_viewport(1920, 0, 16.0 / 9.0), None);
        assert_eq!(letterbox_viewport(1920, 1080, 0.0), None);
    }
}
