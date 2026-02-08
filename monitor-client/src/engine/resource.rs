use crate::renderer::Texture;
use nalgebra::{Matrix3, Point2, Vector2};
use serde::Deserialize;
use std::collections::HashMap;

pub const NOTE_WIDTH_RATIO_BASE: f32 = 0.13175016;

pub type Matrix = Matrix3<f32>;
pub type Point = Point2<f32>;
pub type Vector = Vector2<f32>;

#[derive(Deserialize, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

pub struct NoteStyle {
    pub click: Texture,
    pub hold: Texture,
    pub flick: Texture,
    pub drag: Texture,
    pub hold_body: Option<Texture>,
    pub hold_atlas: (u32, u32),
}

impl NoteStyle {
    pub fn new(
        click: Texture,
        hold: Texture,
        flick: Texture,
        drag: Texture,
        hold_atlas: (u32, u32),
    ) -> Self {
        Self {
            click,
            hold,
            flick,
            drag,
            hold_body: None,
            hold_atlas,
        }
    }

    // Helper to calculate UVs for hold parts
    pub fn hold_head_rect(&self) -> Rect {
        let sy = self.hold_atlas.1 as f32 / self.hold.height as f32;
        Rect::new(0., 1. - sy, 1., sy)
    }

    pub fn hold_body_rect(&self) -> Rect {
        let sy = self.hold_atlas.0 as f32 / self.hold.height as f32;
        let ey = 1. - self.hold_atlas.1 as f32 / self.hold.height as f32;
        Rect::new(0., sy, 1., ey - sy)
    }

    pub fn hold_tail_rect(&self) -> Rect {
        let ey = self.hold_atlas.0 as f32 / self.hold.height as f32;
        Rect::new(0., 0., 1., ey)
    }

    pub fn hold_ratio(&self) -> f32 {
        self.hold.height as f32 / self.hold.width as f32
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResPackInfo {
    pub hold_atlas: (u32, u32),
}

pub struct ResourcePack {
    pub info: ResPackInfo,
    pub note_style: NoteStyle,
}

pub struct Resource {
    pub model_stack: Vec<Matrix>,
    pub textures: HashMap<u32, Texture>,
    pub time: f32,
    pub width: u32,
    pub height: u32,
    pub res_pack: Option<ResourcePack>,
    pub aspect_ratio: f32,
    pub note_width: f32,
}

impl Resource {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            model_stack: vec![Matrix::identity()],
            textures: HashMap::new(),
            time: 0.0,
            width,
            height,
            res_pack: None,
            aspect_ratio: width as f32 / height as f32,
            note_width: NOTE_WIDTH_RATIO_BASE,
        }
    }

    pub fn load_defaults(
        &mut self,
        ctx: &crate::renderer::GlContext,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let click = crate::renderer::Texture::create_solid_color(ctx, 64, 16, [0, 255, 255, 255])?;
        let drag = crate::renderer::Texture::create_solid_color(ctx, 64, 16, [255, 255, 0, 255])?;
        let flick = crate::renderer::Texture::create_solid_color(ctx, 64, 16, [255, 0, 0, 255])?;
        let hold = crate::renderer::Texture::create_solid_color(
            ctx,
            16,
            16,
            [0, 255, 255, 180], // Semi-transparent for hold body? User said "blue long rect". Solid is safer. 180 alpha.
        )?;

        let style = NoteStyle::new(
            click,
            hold,
            flick,
            drag,
            (0, 0), // (0, 0) atlas means full body, no head/tail
        );

        self.res_pack = Some(ResourcePack {
            info: ResPackInfo { hold_atlas: (1, 1) },
            note_style: style,
        });
        Ok(())
    }

    pub fn push_model(&mut self, transform: Matrix) {
        let current = *self.model_stack.last().unwrap();
        self.model_stack.push(current * transform);
    }

    pub fn pop_model(&mut self) {
        if self.model_stack.len() > 1 {
            self.model_stack.pop();
        }
    }

    pub fn current_model(&self) -> Matrix {
        *self.model_stack.last().unwrap()
    }

    pub fn transform_point(&self, p: Point) -> Point {
        self.model_stack.last().unwrap().transform_point(&p)
    }

    pub fn get_gl_matrix(&self) -> [f32; 16] {
        let m = self.model_stack.last().unwrap();
        [
            m[(0, 0)],
            m[(1, 0)],
            0.0,
            0.0,
            m[(0, 1)],
            m[(1, 1)],
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            m[(0, 2)],
            m[(1, 2)],
            0.0,
            1.0,
        ]
    }

    pub fn with_model<F>(&mut self, transform: Matrix, f: F)
    where
        F: FnOnce(&mut Self),
    {
        self.push_model(transform);
        f(self);
        self.pop_model();
    }
}
