use crate::renderer::Texture;
use anyhow::Result;
use nalgebra::{Matrix3, Point2, Vector2};
use serde::Deserialize;
use std::collections::HashMap;

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

#[inline]
fn default_scale() -> f32 {
    1.
}

#[inline]
fn default_duration() -> f32 {
    0.5
}

#[inline]
fn default_perfect() -> u32 {
    0xe1ffec9f
}

#[inline]
fn default_good() -> u32 {
    0xebb4e1ff
}

#[inline]
fn default_tinted() -> bool {
    true
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

    pub fn hold_head_rect(&self) -> Rect {
        let sy = self.hold_atlas.1 as f32 / self.hold.height as f32;
        Rect::new(0., 1. - sy, 1., sy)
    }

    pub fn hold_body_rect(&self) -> Rect {
        let sy = self.hold_atlas.1 as f32 / self.hold.height as f32;
        let ey = self.hold_atlas.0 as f32 / self.hold.height as f32;

        Rect::new(0., ey, 1., 1. - sy - ey)
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
    pub name: String,
    pub author: String,
    pub description: String,
    pub hold_atlas: (u32, u32),
    #[serde(rename = "holdAtlasMH")]
    pub hold_atlas_mh: (u32, u32),
    #[serde(default)]
    pub hold_repeat: bool,
    #[serde(default)]
    pub hold_compact: bool,

    pub hit_fx: (u32, u32),
    #[serde(default = "default_duration")]
    pub hit_fx_duration: f32,
    #[serde(default = "default_scale")]
    pub hit_fx_scale: f32,
    #[serde(default)]
    pub hit_fx_rotate: bool,
    #[serde(default)]
    pub hide_particles: bool,
    #[serde(default = "default_tinted")]
    pub hit_fx_tinted: bool,

    #[serde(default = "default_perfect")]
    pub color_perfect: u32,
    #[serde(default = "default_good")]
    pub color_good: u32,
}

impl ResPackInfo {
    pub fn fx_perfect(&self) -> monitor_common::core::Color {
        if self.hit_fx_tinted {
            monitor_common::core::Color::from_hex(self.color_perfect)
        } else {
            monitor_common::core::colors::WHITE
        }
    }

    pub fn fx_good(&self) -> monitor_common::core::Color {
        if self.hit_fx_tinted {
            monitor_common::core::Color::from_hex(self.color_good)
        } else {
            monitor_common::core::colors::WHITE
        }
    }
}

pub struct ResourcePack {
    pub info: ResPackInfo,
    pub note_style: NoteStyle,
    pub note_style_mh: NoteStyle,
    pub hit_fx: Texture,
}

impl ResourcePack {
    pub async fn load(
        ctx: &crate::renderer::GlContext,
        files: HashMap<String, Vec<u8>>,
    ) -> Result<Self, anyhow::Error> {
        let info_bytes = files
            .get("info.yml")
            .ok_or_else(|| anyhow::anyhow!("Missing info.yml"))?;
        let info_str = String::from_utf8(info_bytes.clone())?;
        let info: ResPackInfo = serde_yaml::from_str(&info_str)?;

        // Helper to load texture from bytes
        async fn load_tex(
            ctx: &crate::renderer::GlContext,
            files: &HashMap<String, Vec<u8>>,
            name: &str,
        ) -> Result<Texture, anyhow::Error> {
            let bytes = files
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Missing {}", name))?;
            Ok(Texture::load_from_bytes(ctx, bytes)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to load texture {}: {:?}", name, e))?)
        }

        let note_style = NoteStyle::new(
            load_tex(ctx, &files, "click.png").await?,
            load_tex(ctx, &files, "hold.png").await?,
            load_tex(ctx, &files, "flick.png").await?,
            load_tex(ctx, &files, "drag.png").await?,
            info.hold_atlas,
        );

        let note_style_mh = NoteStyle::new(
            load_tex(ctx, &files, "click_mh.png").await?,
            load_tex(ctx, &files, "hold_mh.png").await?,
            load_tex(ctx, &files, "flick_mh.png").await?,
            load_tex(ctx, &files, "drag_mh.png").await?,
            info.hold_atlas_mh,
        );

        // TODO: Handle hold_repeat body generation if needed

        let hit_fx = load_tex(ctx, &files, "hit_fx.png")
            .await
            .unwrap_or_else(|e| {
                web_sys::console::log_1(&format!("Failed to load hit_fx.png: {:?}", e).into());
                Texture::create_solid_color(ctx, 64, 64, [255, 255, 255, 255]).unwrap()
            });

        Ok(Self {
            info,
            note_style,
            note_style_mh,
            hit_fx,
        })
    }
}

pub struct Resource {
    pub model_stack: Vec<Matrix>,
    pub textures: HashMap<u32, Texture>,
    pub time: f32,
    pub dt: f32,
    pub width: u32,
    pub height: u32,
    pub res_pack: Option<ResourcePack>,
    pub aspect_ratio: f32,
    pub note_width: f32,
    pub note_scale: f32,
    pub line_textures: HashMap<usize, Texture>,
    pub emitter: Option<ParticleEmitter>,
}

pub struct ParticleEmitter {
    pub scale: f32,
    pub emitter: crate::renderer::particle::Emitter,
    pub emitter_square: crate::renderer::particle::Emitter,
    pub hide_particles: bool,
}

impl ParticleEmitter {
    pub fn new(
        ctx: &crate::renderer::GlContext,
        res_pack: &ResourcePack,
        scale: f32,
        hide_particles: bool,
    ) -> Result<Self, String> {
        use crate::renderer::particle::{AtlasConfig, ColorCurve, Emitter, EmitterConfig};
        use monitor_common::core::colors;

        let colors_curve = {
            let start = colors::WHITE;
            let mut mid = start;
            let mut end = start;
            mid.a *= 0.7;
            end.a = 0.;
            ColorCurve { start, mid, end }
        };

        let mut res = Self {
            scale: res_pack.info.hit_fx_scale,
            emitter: Emitter::new(
                ctx,
                EmitterConfig {
                    local_coords: false,
                    texture: Some(res_pack.hit_fx.clone()),
                    lifetime: res_pack.info.hit_fx_duration,
                    lifetime_randomness: 0.0,
                    initial_rotation_randomness: 0.0,
                    initial_direction_spread: 0.0,
                    initial_velocity: 0.0,
                    size: 0.3, // Reduced from implicit default 1.0 (too big)
                    atlas: Some(AtlasConfig::new(
                        res_pack.info.hit_fx.0 as _,
                        res_pack.info.hit_fx.1 as _,
                        0,
                        (res_pack.info.hit_fx.0 * res_pack.info.hit_fx.1) as _,
                    )),
                    emitting: false,
                    colors_curve,
                    blend_mode: crate::renderer::particle::BlendMode::Alpha, // Changed to Alpha for debugging
                    ..Default::default()
                },
            )?,
            emitter_square: Emitter::new(
                ctx,
                EmitterConfig {
                    local_coords: false,
                    lifetime: res_pack.info.hit_fx_duration,
                    lifetime_randomness: 0.0,
                    initial_direction_spread: 2. * std::f32::consts::PI,
                    size_randomness: 0.3,
                    emitting: false,
                    initial_velocity: 2.5 * scale,
                    initial_velocity_randomness: 1. / 10.,
                    linear_accel: -6. / 1.,
                    colors_curve,
                    blend_mode: crate::renderer::particle::BlendMode::Alpha,
                    ..Default::default()
                },
            )?,
            hide_particles,
        };
        res.set_scale(scale);
        Ok(res)
    }

    pub fn emit_at(&mut self, pt: Vector, rotation: f32, color: monitor_common::core::Color) {
        self.emitter.config.initial_rotation = rotation;
        self.emitter.config.base_color = color;
        self.emitter.emit(pt, 1);
        if !self.hide_particles {
            self.emitter_square.config.base_color = color;
            self.emitter_square.emit(pt, 4);
        }
    }

    pub fn draw(&mut self, renderer: &mut crate::renderer::Renderer, dt: f32) {
        self.emitter.draw(
            &renderer.context,
            Vector::new(0., 0.),
            dt,
            &renderer.projection,
            &renderer.white_texture,
        );
        self.emitter_square.draw(
            &renderer.context,
            Vector::new(0., 0.),
            dt,
            &renderer.projection,
            &renderer.white_texture,
        );

        // Invalidate texture cache because we tampered with Unit 0
        renderer.batcher.invalidate_texture_cache();
    }

    pub fn set_scale(&mut self, scale: f32) {
        let base_width = monitor_common::core::NOTE_WIDTH_RATIO_BASE * 2.0;
        self.emitter.config.size = self.scale * scale * base_width;
        // Keep square size calculation from phira
        self.emitter_square.config.size = self.scale * scale * base_width / 8.8;
        self.emitter_square.config.initial_velocity = 2.5 * scale;
    }
}

impl Resource {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            model_stack: vec![Matrix::identity()],
            textures: HashMap::new(),
            time: 0.0,
            dt: 0.0,
            width,
            height,
            res_pack: None,
            aspect_ratio: width as f32 / height as f32,
            note_width: monitor_common::core::NOTE_WIDTH_RATIO_BASE,
            note_scale: 1.0,
            line_textures: HashMap::new(),
            emitter: None,
        }
    }

    // Initialize with a default/"fallback" resource pack (solid colors)
    // This is useful if no pack is loaded yet, but usually we want to load one.
    // For now, let's keep it but ideally we load real textures.
    pub fn load_defaults(
        &mut self,
        ctx: &crate::renderer::GlContext,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let click = crate::renderer::Texture::create_solid_color(ctx, 64, 16, [0, 255, 255, 255])?;
        let drag = crate::renderer::Texture::create_solid_color(ctx, 64, 16, [255, 255, 0, 255])?;
        let flick = crate::renderer::Texture::create_solid_color(ctx, 64, 16, [255, 0, 0, 255])?;
        let hold = crate::renderer::Texture::create_solid_color(ctx, 16, 16, [0, 255, 255, 180])?;

        let style = NoteStyle::new(
            click.clone(),
            hold.clone(),
            flick.clone(),
            drag.clone(),
            (0, 0),
        );

        let style_mh = NoteStyle::new(click, hold, flick, drag, (0, 0));

        let res_pack = ResourcePack {
            info: ResPackInfo {
                name: "fallback".to_string(),
                author: "monitor".to_string(),
                description: "fallback".to_string(),
                hold_atlas: (1, 1),
                hold_atlas_mh: (1, 1),
                hold_repeat: false,
                hold_compact: false,

                hit_fx: (1, 1),
                hit_fx_duration: 0.5,
                hit_fx_scale: 1.0,
                hit_fx_rotate: false,
                hide_particles: false,
                hit_fx_tinted: true,
                color_perfect: 0xe1ffec9f,
                color_good: 0xebb4e1ff,
            },
            note_style: style,
            note_style_mh: style_mh,
            hit_fx: crate::renderer::Texture::create_solid_color(ctx, 1, 1, [255, 255, 255, 255])?,
        };

        self.set_pack(ctx, res_pack)?;

        Ok(())
    }

    pub fn set_pack(
        &mut self,
        ctx: &crate::renderer::GlContext,
        pack: ResourcePack,
    ) -> Result<(), String> {
        self.emitter = Some(ParticleEmitter::new(ctx, &pack, self.note_scale, false)?);
        self.res_pack = Some(pack);
        Ok(())
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.note_scale = scale;
        if let Some(emitter) = &mut self.emitter {
            emitter.set_scale(scale);
        }
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

    pub fn emit_at_origin(&mut self, rotation: f32, color: monitor_common::core::Color) {
        let model = self.current_model();
        if let Some(emitter) = &mut self.emitter {
            let pt = model.transform_point(&nalgebra::Point2::origin());
            let vec = Vector2::new(pt.x, pt.y);
            emitter.emit_at(vec, rotation, color);
        }
    }
}
