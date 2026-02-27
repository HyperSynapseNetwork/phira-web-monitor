use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext, WebGlProgram, WebGlShader};

pub struct GlContext {
    pub gl: WebGl2RenderingContext,
    pub width: u32,
    pub height: u32,
}

impl GlContext {
    pub fn new(canvas_id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("no global `window` exists")?;
        let document = window
            .document()
            .ok_or("should have a document on window")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or(format!("canvas element '{}' not found", canvas_id))?
            .dyn_into::<HtmlCanvasElement>()?;

        let gl = canvas
            .get_context("webgl2")?
            .ok_or("WebGL 2.0 not supported")?
            .dyn_into::<WebGl2RenderingContext>()?;

        // Enable blending
        gl.enable(WebGl2RenderingContext::BLEND);
        gl.blend_func(
            WebGl2RenderingContext::SRC_ALPHA,
            WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
        );

        let width = canvas.width();
        let height = canvas.height();
        gl.viewport(0, 0, width as i32, height as i32);

        Ok(Self { gl, width, height })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.gl.viewport(0, 0, width as i32, height as i32);
    }

    pub fn set_viewport(&mut self, x: i32, y: i32, width: u32, height: u32) {
        self.gl.viewport(x, y, width as i32, height as i32);
    }

    pub fn clear(&self, r: f32, g: f32, b: f32, a: f32) {
        self.gl.clear_color(r, g, b, a);
        self.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    }

    pub fn create_shader(&self, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
        let shader = self
            .gl
            .create_shader(shader_type)
            .ok_or_else(|| String::from("Unable to create shader object"))?;
        self.gl.shader_source(&shader, source);
        self.gl.compile_shader(&shader);

        if self
            .gl
            .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            Ok(shader)
        } else {
            Err(self
                .gl
                .get_shader_info_log(&shader)
                .unwrap_or_else(|| String::from("Unknown error creating shader")))
        }
    }

    pub fn create_program(
        &self,
        vert: &WebGlShader,
        frag: &WebGlShader,
    ) -> Result<WebGlProgram, String> {
        let program = self
            .gl
            .create_program()
            .ok_or_else(|| String::from("Unable to create shader program"))?;
        self.gl.attach_shader(&program, vert);
        self.gl.attach_shader(&program, frag);
        self.gl.link_program(&program);

        if self
            .gl
            .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            Ok(program)
        } else {
            Err(self
                .gl
                .get_program_info_log(&program)
                .unwrap_or_else(|| String::from("Unknown error creating program")))
        }
    }
}
