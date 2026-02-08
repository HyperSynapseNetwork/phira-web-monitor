use std::fmt::format;

use super::context::GlContext;
use super::texture::Texture;
use wasm_bindgen::prelude::*;
use web_sys::{WebGl2RenderingContext, WebGlBuffer, WebGlVertexArrayObject};

const MAX_QUADS: usize = 10000;
const VERTICES_PER_QUAD: usize = 4;
const INDICES_PER_QUAD: usize = 6;
const FLOATS_PER_VERTEX: usize = 8; // x, y, u, v, r, g, b, a

pub struct Batcher {
    vertices: Vec<f32>,
    indices: Vec<u16>,
    vbo: WebGlBuffer,
    ebo: WebGlBuffer,
    vao: WebGlVertexArrayObject,
    index_count: i32,
    active_texture_id: Option<u32>,
}

impl Batcher {
    pub fn new(ctx: &GlContext) -> Result<Self, JsValue> {
        let vao = ctx.gl.create_vertex_array().ok_or("failed to create VAO")?;
        ctx.gl.bind_vertex_array(Some(&vao));

        let vbo = ctx.gl.create_buffer().ok_or("failed to create VBO")?;
        ctx.gl
            .bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));

        // Allocate empty buffer
        let size = (MAX_QUADS * VERTICES_PER_QUAD * FLOATS_PER_VERTEX * 4) as i32;
        ctx.gl.buffer_data_with_i32(
            WebGl2RenderingContext::ARRAY_BUFFER,
            size,
            WebGl2RenderingContext::DYNAMIC_DRAW,
        );

        let ebo = ctx.gl.create_buffer().ok_or("failed to create EBO")?;
        ctx.gl
            .bind_buffer(WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER, Some(&ebo));

        // Layout
        let stride = (FLOATS_PER_VERTEX * 4) as i32;

        // Pos
        ctx.gl.vertex_attrib_pointer_with_i32(
            0,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            0,
        );
        ctx.gl.enable_vertex_attrib_array(0);

        // UV
        ctx.gl.vertex_attrib_pointer_with_i32(
            1,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            2 * 4,
        );
        ctx.gl.enable_vertex_attrib_array(1);

        // Color
        ctx.gl.vertex_attrib_pointer_with_i32(
            2,
            4,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            4 * 4,
        );
        ctx.gl.enable_vertex_attrib_array(2);

        // Pre-fill indices
        let mut indices = Vec::with_capacity(MAX_QUADS * INDICES_PER_QUAD);
        for i in 0..MAX_QUADS {
            let base = (i * 4) as u16;
            indices.push(base + 0);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base + 0);
            indices.push(base + 2);
            indices.push(base + 3);
        }

        let indices_view = unsafe { js_sys::Uint16Array::view(&indices) };
        ctx.gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            &indices_view,
            WebGl2RenderingContext::STATIC_DRAW,
        );

        ctx.gl.bind_vertex_array(None);

        Ok(Self {
            vertices: Vec::with_capacity(MAX_QUADS * VERTICES_PER_QUAD * FLOATS_PER_VERTEX),
            indices,
            vbo,
            ebo,
            vao,
            index_count: 0,
            active_texture_id: None,
        })
    }

    pub fn set_texture(&mut self, ctx: &GlContext, texture: &Texture) {
        if self.active_texture_id != Some(texture.id) {
            self.flush(ctx);
            self.active_texture_id = Some(texture.id);
            ctx.gl
                .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture.texture));
        }
    }

    pub fn draw_rect(
        &mut self,
        ctx: &GlContext,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        model: &[f32; 16],
    ) {
        if self.vertices.len() + VERTICES_PER_QUAD * FLOATS_PER_VERTEX > self.vertices.capacity() {
            self.flush(ctx);
        }

        let coords = [
            (x, y),         // 0
            (x + w, y),     // 1
            (x + w, y + h), // 2
            (x, y + h),     // 3
        ];

        for (vx, vy) in coords {
            let tx = model[0] * vx + model[4] * vy + model[12];
            let ty = model[1] * vx + model[5] * vy + model[13];
            self.vertices
                .extend_from_slice(&[tx, ty, 0.0, 0.0, r, g, b, a]);
        }

        self.index_count += INDICES_PER_QUAD as i32;
    }

    pub fn draw_texture_rect(
        &mut self,
        ctx: &GlContext,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u: f32,
        v: f32,
        uw: f32,
        uh: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        model: &[f32; 16],
    ) {
        if self.vertices.len() + VERTICES_PER_QUAD * FLOATS_PER_VERTEX > self.vertices.capacity() {
            self.flush(ctx);
        }

        let coords = [
            (x, y, u, v),                   // 0
            (x + w, y, u + uw, v),          // 1
            (x + w, y + h, u + uw, v + uh), // 2
            (x, y + h, u, v + uh),          // 3
        ];

        for (vx, vy, vu, vv) in coords {
            let tx = model[0] * vx + model[4] * vy + model[12];
            let ty = model[1] * vx + model[5] * vy + model[13];
            // web_sys::console::log_1(&format!("({}, {})", tx, ty).into());

            self.vertices
                .extend_from_slice(&[tx, ty, vu, vv, r, g, b, a]);
        }

        self.index_count += INDICES_PER_QUAD as i32;
    }

    pub fn flush(&mut self, ctx: &GlContext) {
        if self.index_count == 0 {
            return;
        }

        ctx.gl.bind_vertex_array(Some(&self.vao));
        ctx.gl
            .bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.vbo));

        let vertices_view = unsafe { js_sys::Float32Array::view(&self.vertices) };
        ctx.gl.buffer_sub_data_with_i32_and_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            0,
            &vertices_view,
        );

        ctx.gl.draw_elements_with_i32(
            WebGl2RenderingContext::TRIANGLES,
            self.index_count,
            WebGl2RenderingContext::UNSIGNED_SHORT,
            0,
        );

        ctx.gl.bind_vertex_array(None);

        self.vertices.clear();
        self.index_count = 0;
    }
}
