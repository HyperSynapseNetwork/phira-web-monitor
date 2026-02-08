use super::context::GlContext;
use std::collections::HashMap;
use web_sys::{WebGl2RenderingContext, WebGlProgram, WebGlUniformLocation};

pub struct ShaderManager {
    programs: HashMap<String, WebGlProgram>,
    current_program: Option<String>,
}

impl ShaderManager {
    pub fn new(_ctx: &GlContext) -> Self {
        Self {
            programs: HashMap::new(),
            current_program: None,
        }
    }

    pub fn init_defaults(&mut self, ctx: &GlContext) -> Result<(), String> {
        let vert_src = r#"#version 300 es
        layout(location = 0) in vec2 a_position;
        layout(location = 1) in vec2 a_tex_coord;
        layout(location = 2) in vec4 a_color;
        
        uniform mat4 u_projection;
        
        out vec2 v_tex_coord;
        out vec4 v_color;
        
        void main() {
            gl_Position = u_projection * vec4(a_position, 0.0, 1.0);
            v_tex_coord = a_tex_coord;
            v_color = a_color;
        }
        "#;

        let frag_src = r#"#version 300 es
        precision mediump float;
        
        in vec2 v_tex_coord;
        in vec4 v_color;
        
        uniform sampler2D u_texture;
        
        out vec4 out_color;
        
        void main() {
            out_color = texture(u_texture, v_tex_coord) * v_color;
        }
        "#;

        let vert = ctx.create_shader(WebGl2RenderingContext::VERTEX_SHADER, vert_src)?;
        let frag = ctx.create_shader(WebGl2RenderingContext::FRAGMENT_SHADER, frag_src)?;
        let program = ctx.create_program(&vert, &frag)?;

        self.programs.insert("default".to_string(), program);

        Ok(())
    }

    pub fn use_program(&mut self, ctx: &GlContext, name: &str) {
        if let Some(program) = self.programs.get(name) {
            ctx.gl.use_program(Some(program));
            self.current_program = Some(name.to_string());
        }
    }

    pub fn set_uniform_matrix4fv(&self, ctx: &GlContext, name: &str, val: &[f32]) {
        let loc = self.get_uniform_location(ctx, self.current_program.as_ref().unwrap(), name);
        if let Some(loc) = loc {
            ctx.gl
                .uniform_matrix4fv_with_f32_array(Some(&loc), false, val);
        }
    }

    pub fn get_uniform_location(
        &self,
        ctx: &GlContext,
        _: &str,
        uniform: &str,
    ) -> Option<WebGlUniformLocation> {
        if let Some(name) = &self.current_program {
            if let Some(program) = self.programs.get(name) {
                return ctx.gl.get_uniform_location(program, uniform);
            }
        }
        None
    }
}
