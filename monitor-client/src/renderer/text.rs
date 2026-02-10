use crate::renderer::{Renderer, Texture};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Glyph {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub advance: f32,
}

#[derive(Clone)]
pub struct SpriteFont {
    pub texture: Texture,
    pub map: HashMap<char, Glyph>,
    pub line_height: f32,
}

impl SpriteFont {
    pub fn new(texture: Texture, line_height: f32) -> Self {
        Self {
            texture,
            map: HashMap::new(),
            line_height,
        }
    }

    pub fn add_glyph(&mut self, c: char, glyph: Glyph) {
        self.map.insert(c, glyph);
    }

    // Simplified monospace grid loader
    pub fn load_grid(&mut self, chars: &str, cols: u32, rows: u32, cell_w: f32, cell_h: f32) {
        let tex_w = self.texture.width as f32;
        let tex_h = self.texture.height as f32;

        // UV size
        let u_step = cell_w / tex_w;
        let v_step = cell_h / tex_h;

        for (i, c) in chars.chars().enumerate() {
            let col = (i as u32) % cols;
            let row = (i as u32) / cols;

            if row >= rows {
                break;
            }

            let u = col as f32 * u_step;
            let v = row as f32 * v_step;

            self.add_glyph(
                c,
                Glyph {
                    x: u,
                    y: v,
                    w: u_step,
                    h: v_step,
                    advance: cell_w, // Monospace for now? Or pass width?
                },
            );
        }
    }

    pub fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| {
                self.map
                    .get(&c)
                    .map_or(0.0, |g| g.advance * (size / self.line_height))
            })
            .sum()
    }

    pub fn draw_text_color(
        &self,
        renderer: &mut Renderer,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        align: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        model: &[f32; 16],
    ) {
        if text.is_empty() {
            return;
        }

        let total_width = self.width(text, size);
        let mut cursor_x = x - total_width * align;
        let cursor_y = y;

        renderer.set_texture(&self.texture);

        for c in text.chars() {
            if let Some(glyph) = self.map.get(&c) {
                let w = glyph.w;
                let h = glyph.h;

                let scale = size / self.line_height;
                let draw_w = glyph.advance * scale;
                let draw_h = size;

                renderer.draw_texture_rect(
                    cursor_x, cursor_y, draw_w, draw_h, glyph.x, glyph.y, w, h, r, g, b, a, model,
                );

                cursor_x += draw_w;
            }
        }
    }

    pub fn draw_text(
        &self,
        renderer: &mut Renderer,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        align: f32,
        model: &[f32; 16],
    ) {
        self.draw_text_color(renderer, text, x, y, size, align, 1.0, 1.0, 1.0, 1.0, model);
    }
}
