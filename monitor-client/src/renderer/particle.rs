use crate::renderer::{GlContext, Texture};
use monitor_common::core::Color;
use monitor_common::core::colors;
use nalgebra::Vector2;
use web_sys::{WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlVertexArrayObject};

#[derive(Clone, Copy, Debug)]
pub enum EmissionShape {
    Point,
    Rect { width: f32, height: f32 },
    Sphere { radius: f32 },
}

impl EmissionShape {
    fn gen_random_point(&self) -> Vector2<f32> {
        match self {
            EmissionShape::Point => Vector2::new(0.0, 0.0),
            EmissionShape::Rect { width, height } => {
                let x = (js_sys::Math::random() as f32 - 0.5) * width;
                let y = (js_sys::Math::random() as f32 - 0.5) * height;
                Vector2::new(x, y)
            }
            EmissionShape::Sphere { radius } => {
                let ro = (js_sys::Math::random() as f32 * radius * radius).sqrt();
                let phi = js_sys::Math::random() as f32 * std::f32::consts::PI * 2.0;
                Vector2::new(ro * phi.cos(), ro * phi.sin())
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ColorCurve {
    pub start: Color,
    pub mid: Color,
    pub end: Color,
}

impl Default for ColorCurve {
    fn default() -> Self {
        Self {
            start: colors::WHITE,
            mid: colors::WHITE,
            end: colors::WHITE,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AtlasConfig {
    pub n: u16,
    pub m: u16,
    pub start_index: u16,
    pub end_index: u16,
}

impl AtlasConfig {
    pub fn new(n: u16, m: u16, start: u16, end: u16) -> Self {
        Self {
            n,
            m,
            start_index: start,
            end_index: end,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BlendMode {
    Alpha,
    Add,
}

#[derive(Clone, Debug)]
pub struct EmitterConfig {
    pub local_coords: bool,
    pub emission_shape: EmissionShape,
    pub lifetime: f32,
    pub lifetime_randomness: f32,
    pub amount: u32,
    pub explosiveness: f32,
    pub initial_direction: Vector2<f32>,
    pub initial_direction_spread: f32,
    pub initial_velocity: f32,
    pub initial_velocity_randomness: f32,
    pub linear_accel: f32,
    pub initial_rotation: f32,
    pub initial_rotation_randomness: f32,
    pub initial_angular_velocity: f32,
    pub initial_angular_velocity_randomness: f32,
    pub angular_accel: f32,
    pub angular_damping: f32,
    pub size: f32,
    pub size_randomness: f32,
    pub texture: Option<Texture>,
    pub atlas: Option<AtlasConfig>,
    pub base_color: Color,
    pub colors_curve: ColorCurve,
    pub gravity: Vector2<f32>,
    pub emitting: bool,
    pub one_shot: bool,
    pub blend_mode: BlendMode,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            local_coords: false,
            emission_shape: EmissionShape::Point,
            lifetime: 1.0,
            lifetime_randomness: 0.0,
            amount: 8,
            explosiveness: 0.0,
            initial_direction: Vector2::new(0.0, -1.0),
            initial_direction_spread: 0.0,
            initial_velocity: 50.0,
            initial_velocity_randomness: 0.0,
            linear_accel: 0.0,
            initial_rotation: 0.0,
            initial_rotation_randomness: 0.0,
            initial_angular_velocity: 0.0,
            initial_angular_velocity_randomness: 0.0,
            angular_accel: 0.0,
            angular_damping: 0.0,
            size: 10.0,
            size_randomness: 0.0,
            texture: None,
            atlas: None,
            base_color: colors::WHITE,
            colors_curve: ColorCurve::default(),
            gravity: Vector2::new(0.0, 0.0),
            emitting: true,
            one_shot: false,
            blend_mode: BlendMode::Alpha,
        }
    }
}

// Helper for randomness
fn rand_range(min: f32, max: f32) -> f32 {
    min + (max - min) * js_sys::Math::random() as f32
}

struct CpuParticle {
    velocity: Vector2<f32>,
    angular_velocity: f32,
    lived: f32,
    lifetime: f32,
    frame: u16,
    initial_size: f32,
    color: Color,

    // Position offset from emitter origin at spawn time
    offset: Vector2<f32>,
    // Rotation at spawn time
    initial_rotation: f32,
}

pub struct Emitter {
    program: WebGlProgram,
    vao: WebGlVertexArrayObject,
    instance_buffer: WebGlBuffer,

    // Gpu data buffer (f32)
    gpu_data: Vec<f32>,
    cpu_particles: Vec<CpuParticle>,

    pub config: EmitterConfig,

    position: Vector2<f32>,
    particles_spawned: u64,
    last_emit_time: f32,
    time_passed: f32,

    max_particles: usize,
}

impl Emitter {
    const SHADER_VS: &'static str = r#"#version 300 es
        layout(location = 0) in vec3 a_pos;
        layout(location = 1) in vec2 a_uv;
        layout(location = 2) in vec4 a_color;
        layout(location = 3) in vec4 a_inst_pos;
        layout(location = 4) in vec4 a_inst_uv;
        layout(location = 5) in vec4 a_inst_data;
        layout(location = 6) in vec4 a_inst_color;

        uniform mat4 u_mvp;
        uniform float u_local_coords;
        uniform vec3 u_emitter_position;

        out vec2 v_uv;
        out vec4 v_color;

        mat2 rotate2d(float angle) {
            return mat2(cos(angle), -sin(angle),
                        sin(angle), cos(angle));
        }

        void main() {
            float rotation = a_inst_pos.z;
            float size = a_inst_pos.w;
            vec2 pos = a_inst_pos.xy;

            mat2 rot = rotate2d(rotation);
            vec2 transformed_pos = rot * a_pos.xy * size;
            
            vec3 final_pos;
            if (u_local_coords == 0.0) {
                final_pos = vec3(transformed_pos + pos, a_pos.z);
            } else {
                final_pos = vec3(transformed_pos + pos + u_emitter_position.xy, a_pos.z);
            }

            gl_Position = u_mvp * vec4(final_pos, 1.0);
            v_uv = a_uv * a_inst_uv.zw + a_inst_uv.xy;
            v_color = a_inst_color;
        }
    "#;

    const SHADER_FS: &'static str = r#"#version 300 es
        precision mediump float;
        in vec2 v_uv;
        in vec4 v_color;
        uniform sampler2D u_texture;
        out vec4 out_color;
        void main() {
            out_color = texture(u_texture, v_uv) * v_color;
        }
    "#;

    pub fn new(ctx: &GlContext, config: EmitterConfig) -> Result<Self, String> {
        let gl = &ctx.gl;
        let max_particles = 12000;

        // Compile Shader
        let vert = ctx.create_shader(WebGl2RenderingContext::VERTEX_SHADER, Self::SHADER_VS)?;
        let frag = ctx.create_shader(WebGl2RenderingContext::FRAGMENT_SHADER, Self::SHADER_FS)?;
        let program = ctx.create_program(&vert, &frag)?;

        // VAO
        let vao = gl.create_vertex_array().ok_or("Failed to create VAO")?;
        gl.bind_vertex_array(Some(&vao));

        // Quad Buffer (Static)
        let quad_verts: [f32; 20] = [
            // pos         uv
            -0.5, -0.5, 0.0, 0.0, 1.0, 0.5, -0.5, 0.0, 1.0, 1.0, 0.5, 0.5, 0.0, 1.0, 0.0, -0.5, 0.5,
            0.0, 0.0, 0.0,
        ];
        let quad_buffer = gl.create_buffer().ok_or("Failed to create quad buffer")?;
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&quad_buffer));
        unsafe {
            let view = js_sys::Float32Array::view(&quad_verts);
            gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER,
                &view,
                WebGl2RenderingContext::STATIC_DRAW,
            );
        }

        // Attributes for Quad
        // 0: pos (3)
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 3, WebGl2RenderingContext::FLOAT, false, 20, 0);
        // 1: uv (2)
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(1, 2, WebGl2RenderingContext::FLOAT, false, 20, 12);
        // 2: color (4) - unused in quad buffer, disable or default
        gl.disable_vertex_attrib_array(2);

        // Instance Buffer (Dynamic)
        let instance_buffer = gl
            .create_buffer()
            .ok_or("Failed to create instance buffer")?;
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&instance_buffer));
        // Allocate size
        gl.buffer_data_with_i32(
            WebGl2RenderingContext::ARRAY_BUFFER,
            (max_particles * 16 * 4) as i32, // 16 floats per particle * 4 bytes
            WebGl2RenderingContext::STREAM_DRAW,
        );

        let stride = 16 * 4; // 16 floats * 4 bytes
        // 3: inst_pos (4)
        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_with_i32(3, 4, WebGl2RenderingContext::FLOAT, false, stride, 0);
        gl.vertex_attrib_divisor(3, 1);

        // 4: inst_uv (4)
        gl.enable_vertex_attrib_array(4);
        gl.vertex_attrib_pointer_with_i32(4, 4, WebGl2RenderingContext::FLOAT, false, stride, 16);
        gl.vertex_attrib_divisor(4, 1);

        // 5: inst_data (4)
        gl.enable_vertex_attrib_array(5);
        gl.vertex_attrib_pointer_with_i32(5, 4, WebGl2RenderingContext::FLOAT, false, stride, 32);
        gl.vertex_attrib_divisor(5, 1);

        // 6: inst_color (4)
        gl.enable_vertex_attrib_array(6);
        gl.vertex_attrib_pointer_with_i32(6, 4, WebGl2RenderingContext::FLOAT, false, stride, 48);
        gl.vertex_attrib_divisor(6, 1);

        // Indices
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = gl.create_buffer().ok_or("Failed to create index buffer")?;
        gl.bind_buffer(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            Some(&index_buffer),
        );
        unsafe {
            let view = js_sys::Uint16Array::view(&indices);
            gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
                &view,
                WebGl2RenderingContext::STATIC_DRAW,
            );
        }

        gl.bind_vertex_array(None);

        Ok(Self {
            program,
            vao,
            instance_buffer,
            gpu_data: Vec::with_capacity(max_particles * 16),
            cpu_particles: Vec::with_capacity(max_particles),
            config,
            position: Vector2::new(0.0, 0.0),
            particles_spawned: 0,
            last_emit_time: 0.0,
            time_passed: 0.0,
            max_particles,
        })
    }

    pub fn draw(
        &mut self,
        ctx: &GlContext,
        pos: Vector2<f32>,
        dt: f32,
        mvp: &[f32],
        white_texture: &Texture,
    ) {
        self.position = pos;
        let gl = &ctx.gl;

        self.update(dt); // Updates CPU particles and repopulates gpu_data

        gl.use_program(Some(&self.program));
        gl.bind_vertex_array(Some(&self.vao));

        // Blend Func
        match self.config.blend_mode {
            BlendMode::Alpha => {
                gl.blend_func(
                    WebGl2RenderingContext::SRC_ALPHA,
                    WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
                );
            }
            BlendMode::Add => {
                gl.blend_func(
                    WebGl2RenderingContext::SRC_ALPHA,
                    WebGl2RenderingContext::ONE,
                );
            }
        }

        // Bind Texture
        gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        if let Some(tex) = &self.config.texture {
            gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&tex.texture));
        } else {
            gl.bind_texture(
                WebGl2RenderingContext::TEXTURE_2D,
                Some(&white_texture.texture),
            );
        }

        let u_texture = gl.get_uniform_location(&self.program, "u_texture");
        gl.uniform1i(u_texture.as_ref(), 0);

        // Update Uniforms
        let u_mvp = gl.get_uniform_location(&self.program, "u_mvp");
        gl.uniform_matrix4fv_with_f32_array(u_mvp.as_ref(), false, mvp);
        // Pass u_local_coords
        let u_local_coords = gl.get_uniform_location(&self.program, "u_local_coords");
        gl.uniform1f(
            u_local_coords.as_ref(),
            if self.config.local_coords { 1.0 } else { 0.0 },
        );

        // Pass u_emitter_position
        let u_emitter_pos = gl.get_uniform_location(&self.program, "u_emitter_position");
        gl.uniform3f(
            u_emitter_pos.as_ref(),
            self.position.x,
            self.position.y,
            0.0,
        );

        // Upload Instance Data
        gl.bind_buffer(
            WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.instance_buffer),
        );
        unsafe {
            let view = js_sys::Float32Array::view(&self.gpu_data);
            gl.buffer_sub_data_with_i32_and_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER,
                0,
                &view,
            );
        }

        // Draw
        if !self.cpu_particles.is_empty() {
            gl.draw_elements_instanced_with_i32(
                WebGl2RenderingContext::TRIANGLES,
                6,
                WebGl2RenderingContext::UNSIGNED_SHORT,
                0,
                self.cpu_particles.len() as i32,
            );
        }

        // Cleanup state
        gl.bind_vertex_array(None);
        gl.use_program(None);
        gl.blend_func(
            WebGl2RenderingContext::SRC_ALPHA,
            WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
        );
    }

    // Add set_mvp helper?
    pub fn set_mvp(&self, gl: &WebGl2RenderingContext, mvp: &[f32]) {
        gl.use_program(Some(&self.program));
        let u_mvp = gl.get_uniform_location(&self.program, "u_mvp");
        gl.uniform_matrix4fv_with_f32_array(u_mvp.as_ref(), false, mvp);
    }

    pub fn emit(&mut self, pos: Vector2<f32>, n: usize) {
        for _ in 0..n {
            self.emit_particle(pos);
            self.particles_spawned += 1;
        }
    }

    fn emit_particle(&mut self, offset: Vector2<f32>) {
        if self.cpu_particles.len() >= self.max_particles {
            return;
        }

        let offset = offset + self.config.emission_shape.gen_random_point();

        let initial_direction = self.config.initial_direction;
        let spread = self.config.initial_direction_spread;
        let angle_offset = rand_range(-spread / 2.0, spread / 2.0);

        // Rotate initial_direction by angle_offset
        let cos_a = angle_offset.cos();
        let sin_a = angle_offset.sin();
        let dir_x = initial_direction.x * cos_a - initial_direction.y * sin_a;
        let dir_y = initial_direction.x * sin_a + initial_direction.y * cos_a;
        let dir = Vector2::new(dir_x, dir_y); // Should normalize if config implies unit vector

        let velocity = self.config.initial_velocity
            - self.config.initial_velocity
                * rand_range(0.0, self.config.initial_velocity_randomness);
        let vel_vec = dir * velocity;

        let r = self.config.size - self.config.size * rand_range(0.0, self.config.size_randomness);
        let rotation = self.config.initial_rotation
            - self.config.initial_rotation
                * rand_range(0.0, self.config.initial_rotation_randomness);

        let angular_velocity = self.config.initial_angular_velocity
            - self.config.initial_angular_velocity
                * rand_range(0.0, self.config.initial_angular_velocity_randomness);

        let lifetime = self.config.lifetime
            - self.config.lifetime * rand_range(0.0, self.config.lifetime_randomness);

        self.cpu_particles.push(CpuParticle {
            velocity: vel_vec,
            angular_velocity,
            lived: 0.0,
            lifetime,
            frame: 0,
            initial_size: r,
            color: self.config.base_color,
            offset,
            initial_rotation: rotation,
        });

        self.particles_spawned += 1;
    }

    fn update(&mut self, dt: f32) {
        // Spawning logic
        if self.config.emitting {
            self.time_passed += dt;
            let gap = (self.config.lifetime / self.config.amount as f32)
                * (1.0 - self.config.explosiveness);
            let spawn_amount = if gap < 0.001 {
                self.config.amount as usize
            } else {
                ((self.time_passed - self.last_emit_time) / gap) as usize
            };

            for _ in 0..spawn_amount {
                self.last_emit_time = self.time_passed;
                if self.particles_spawned < self.config.amount as u64 {
                    self.emit_particle(Vector2::new(0.0, 0.0));
                }
                if self.cpu_particles.len() >= self.max_particles {
                    break;
                }
            }
        }

        if self.config.one_shot && self.time_passed > self.config.lifetime {
            self.time_passed = 0.0;
            self.last_emit_time = 0.0;
            self.config.emitting = false;
        }

        // Update physics and prune dead particles
        // We use retain_mut to update elements and remove dead ones
        // But we need to build gpu_data as well.
        // Phira iterates linearly, pushes to gpu_data, and swaps_remove dead ones later.

        self.gpu_data.clear();

        let config = &self.config;

        let mut i = 0;
        while i < self.cpu_particles.len() {
            let p = &mut self.cpu_particles[i];

            p.velocity += p.velocity * config.linear_accel * dt;
            p.angular_velocity += p.angular_velocity * config.angular_accel * dt;
            p.angular_velocity *= 1.0 - config.angular_damping;

            p.offset += p.velocity * dt;
            p.initial_rotation += p.angular_velocity * dt;

            p.lived += dt;
            p.velocity += config.gravity * dt;

            // Remove dead
            if p.lived >= p.lifetime {
                if p.lived != p.lifetime {
                    self.particles_spawned = self.particles_spawned.saturating_sub(1);
                }
                self.cpu_particles.swap_remove(i);
                continue; // Don't increment i
            }

            // Generate GPU Data for this particle
            let t = p.lived / p.lifetime;

            // Color
            let color_vec = if t < 0.5 {
                let t = t * 2.0;
                lerp_color(config.colors_curve.start, config.colors_curve.mid, t)
            } else {
                let t = (t - 0.5) * 2.0;
                lerp_color(config.colors_curve.mid, config.colors_curve.end, t)
            };
            // Multiply by base color (p.color)
            let final_color = Color {
                r: color_vec.r * p.color.r,
                g: color_vec.g * p.color.g,
                b: color_vec.b * p.color.b,
                a: color_vec.a * p.color.a,
            };

            // Pos: x, y, rotation, size
            // size is initial_size * curve(t) (ignoring curve for now)
            let size = p.initial_size;

            // GPU Data Push
            // 3: inst_pos
            self.gpu_data.push(if config.local_coords {
                p.offset.x
            } else {
                self.position.x + p.offset.x
            });
            self.gpu_data.push(if config.local_coords {
                p.offset.y
            } else {
                self.position.y + p.offset.y
            });
            self.gpu_data.push(p.initial_rotation);
            self.gpu_data.push(size);

            // 4: inst_uv
            if let Some(atlas) = &config.atlas {
                let frame_count = (atlas.end_index - atlas.start_index) as f32;
                let current_frame = ((t * frame_count) as u16 + atlas.start_index)
                    .min(atlas.end_index.saturating_sub(1));

                let x = (current_frame % atlas.n) as f32; // Column index
                let y = (current_frame / atlas.n) as f32; // Row index

                let v_offset = y / atlas.m as f32;

                self.gpu_data.push(x / atlas.n as f32); // inst_uv.x (u_offset)
                self.gpu_data.push(v_offset); // inst_uv.y (v_offset)
                self.gpu_data.push(1.0 / atlas.n as f32); // inst_uv.z (u_scale)
                self.gpu_data.push(1.0 / atlas.m as f32); // inst_uv.w (v_scale)
            } else {
                self.gpu_data.push(0.0);
                self.gpu_data.push(0.0);
                self.gpu_data.push(1.0);
                self.gpu_data.push(1.0);
            }

            // 5: inst_data (index, lifetime_progress, 0, 0)
            self.gpu_data.push(0.0);
            self.gpu_data.push(t);
            self.gpu_data.push(0.0);
            self.gpu_data.push(0.0);

            // 6: inst_color
            self.gpu_data.push(final_color.r);
            self.gpu_data.push(final_color.g);
            self.gpu_data.push(final_color.b);
            self.gpu_data.push(final_color.a);

            i += 1;
        }
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}
