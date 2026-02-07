use crate::shaders;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlShader, WebGlTexture,
    WebGlUniformLocation, WebGlVertexArrayObject,
};

const MAX_QUADS: usize = 2000;
const VERTEX_SIZE: usize = 8; // x, y, u, v, r, g, b, a
const FLOATS_PER_QUAD: usize = VERTEX_SIZE * 6;

pub struct Renderer {
    context: WebGl2RenderingContext,
    program: WebGlProgram,
    u_resolution: Option<WebGlUniformLocation>,

    pub width: u32,
    pub height: u32,

    vao: Option<WebGlVertexArrayObject>,
    buffer: Option<WebGlBuffer>,

    batch_data: Vec<f32>,
    vertex_count: usize,

    white_texture: WebGlTexture,
    textures: std::collections::HashMap<u32, WebGlTexture>,
    texture_sizes: std::collections::HashMap<u32, (u32, u32)>, // id -> (width, height)
}

impl Renderer {
    pub fn new(canvas_id: &str) -> Result<Self, JsValue> {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("Canvas element not found"))?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let context = canvas
            .get_context("webgl2")?
            .ok_or_else(|| JsValue::from_str("Unable to retrieve WebGL2 context"))?
            .dyn_into::<WebGl2RenderingContext>()?;

        context.enable(WebGl2RenderingContext::BLEND);
        // Standard alpha blending
        context.blend_func(
            WebGl2RenderingContext::SRC_ALPHA,
            WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
        );

        let vert_shader = compile_shader(
            &context,
            WebGl2RenderingContext::VERTEX_SHADER,
            shaders::VERTEX_SHADER,
        )?;
        let frag_shader = compile_shader(
            &context,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            shaders::FRAGMENT_SHADER,
        )?;

        let program = link_program(&context, &vert_shader, &frag_shader)?;

        let u_resolution = context.get_uniform_location(&program, "u_resolution");

        let a_position = context.get_attrib_location(&program, "a_position") as u32;
        let a_uv = context.get_attrib_location(&program, "a_uv") as u32;
        let a_color = context.get_attrib_location(&program, "a_color") as u32;

        let width = canvas.width();
        let height = canvas.height();
        context.viewport(0, 0, width as i32, height as i32);

        // White texture
        let white_texture = context.create_texture().unwrap();
        context.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&white_texture));
        context.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            WebGl2RenderingContext::TEXTURE_2D,
            0,
            WebGl2RenderingContext::RGBA as i32,
            1,
            1,
            0,
            WebGl2RenderingContext::RGBA,
            WebGl2RenderingContext::UNSIGNED_BYTE,
            Some(&[255, 255, 255, 255]),
        )?;

        let vao = context.create_vertex_array();
        context.bind_vertex_array(vao.as_ref());

        let buffer = context.create_buffer();
        context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, buffer.as_ref());

        context.buffer_data_with_i32(
            WebGl2RenderingContext::ARRAY_BUFFER,
            (MAX_QUADS * FLOATS_PER_QUAD * 4) as i32,
            WebGl2RenderingContext::DYNAMIC_DRAW,
        );

        let stride = (VERTEX_SIZE * 4) as i32;

        // a_position
        context.enable_vertex_attrib_array(a_position);
        context.vertex_attrib_pointer_with_i32(
            a_position,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            0,
        );

        // a_uv
        context.enable_vertex_attrib_array(a_uv);
        context.vertex_attrib_pointer_with_i32(
            a_uv,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            8,
        ); // Offset 8 (2 floats)

        // a_color
        context.enable_vertex_attrib_array(a_color);
        context.vertex_attrib_pointer_with_i32(
            a_color,
            4,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            16,
        ); // Offset 16 (4 floats)

        context.bind_vertex_array(None);

        Ok(Self {
            context,
            program,
            u_resolution,
            width,
            height,
            vao,
            buffer,
            batch_data: Vec::with_capacity(MAX_QUADS * FLOATS_PER_QUAD),
            vertex_count: 0,
            white_texture,
            textures: std::collections::HashMap::new(),
            texture_sizes: std::collections::HashMap::new(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.context.viewport(0, 0, width as i32, height as i32);
    }

    fn flush(&mut self) {
        if self.vertex_count == 0 {
            return;
        }

        let gl = &self.context;
        gl.bind_vertex_array(self.vao.as_ref());
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, self.buffer.as_ref());

        // Upload data
        unsafe {
            let array_buffer_view = js_sys::Float32Array::view(&self.batch_data);
            gl.buffer_sub_data_with_i32_and_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER,
                0,
                &array_buffer_view,
            );
        }

        gl.draw_arrays(
            WebGl2RenderingContext::TRIANGLES,
            0,
            self.vertex_count as i32,
        );

        self.vertex_count = 0;
        self.batch_data.clear();
    }

    pub fn bind_texture(&mut self, texture: Option<&WebGlTexture>) {
        // Simple flush strategy
        self.flush();

        if let Some(tex) = texture {
            self.context
                .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(tex));
        } else {
            self.context.bind_texture(
                WebGl2RenderingContext::TEXTURE_2D,
                Some(&self.white_texture),
            );
        }
    }

    pub fn push_vertex(&mut self, x: f32, y: f32, u: f32, v: f32, r: f32, g: f32, b: f32, a: f32) {
        self.batch_data.push(x);
        self.batch_data.push(y);
        self.batch_data.push(u);
        self.batch_data.push(v);
        self.batch_data.push(r);
        self.batch_data.push(g);
        self.batch_data.push(b);
        self.batch_data.push(a);
    }

    pub fn draw_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rotation: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    ) {
        self.draw_rotated_rect(x, y, w, h, rotation, 0.0, 0.0, 1.0, 1.0, r, g, b, a);
    }

    pub fn draw_rotated_rect(
        &mut self,
        cx: f32,
        cy: f32,
        w: f32,
        h: f32,
        rotation: f32,
        u1: f32,
        v1: f32,
        u2: f32,
        v2: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    ) {
        if self.batch_data.len() + FLOATS_PER_QUAD > self.batch_data.capacity() {
            self.flush();
        }

        let hw = w / 2.0;
        let hh = h / 2.0;

        let x1 = -hw;
        let y1 = -hh;
        let x2 = hw;
        let y2 = -hh;
        let x3 = hw;
        let y3 = hh;
        let x4 = -hw;
        let y4 = hh;

        let (sin, cos) = rotation.sin_cos();

        let transform =
            |x: f32, y: f32| -> (f32, f32) { (cx + x * cos - y * sin, cy + x * sin + y * cos) };

        let (vx1, vy1) = transform(x1, y1);
        let (vx2, vy2) = transform(x2, y2);
        let (vx3, vy3) = transform(x3, y3);
        let (vx4, vy4) = transform(x4, y4);

        // Triangle 1
        self.push_vertex(vx1, vy1, u1, v1, r, g, b, a); // Top Left
        self.push_vertex(vx2, vy2, u2, v1, r, g, b, a); // Top Right
        self.push_vertex(vx3, vy3, u2, v2, r, g, b, a); // Bottom Right

        // Triangle 2
        self.push_vertex(vx1, vy1, u1, v1, r, g, b, a);
        self.push_vertex(vx3, vy3, u2, v2, r, g, b, a);
        self.push_vertex(vx4, vy4, u1, v2, r, g, b, a); // Bottom Left

        self.vertex_count += 6;
    }

    pub fn create_texture(&mut self, id: u32, width: u32, height: u32, data: &[u8]) {
        let texture = self.context.create_texture().unwrap();
        self.context
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));

        // Parameters (Linear filtering)
        self.context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_S,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        self.context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_T,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        self.context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );
        self.context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );

        // Upload
        self.context
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                WebGl2RenderingContext::TEXTURE_2D,
                0,
                WebGl2RenderingContext::RGBA as i32,
                width as i32,
                height as i32,
                0,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                Some(data),
            )
            .unwrap();

        self.textures.insert(id, texture);
        self.texture_sizes.insert(id, (width, height));
    }

    /// Get texture dimensions (width, height) for a given texture ID
    pub fn get_texture_size(&self, id: u32) -> Option<(u32, u32)> {
        self.texture_sizes.get(&id).copied()
    }

    pub fn use_texture(&mut self, id: u32) {
        if let Some(tex) = self.textures.get(&id) {
            let tex_clone = tex.clone();
            self.bind_texture(Some(&tex_clone));
        } else {
            self.bind_texture(None);
        }
    }

    pub fn render(&mut self, _time: f64) {
        let gl = &self.context;

        // Clear
        gl.clear_color(0.1, 0.1, 0.15, 1.0);
        gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);

        gl.use_program(Some(&self.program));

        // Update resolution
        if let Some(loc) = &self.u_resolution {
            gl.uniform2f(Some(loc), self.width as f32, self.height as f32);
        }

        self.flush();
    }
}

fn compile_shader(
    context: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = context
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if context
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(context
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error creating shader")))
    }
}

fn link_program(
    context: &WebGl2RenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = context
        .create_program()
        .ok_or_else(|| String::from("Unable to create shader program"))?;

    context.attach_shader(&program, vert_shader);
    context.attach_shader(&program, frag_shader);
    context.link_program(&program);

    if context
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(context
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error creating program object")))
    }
}
