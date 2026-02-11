use wasm_bindgen::prelude::*;

mod batch;
pub use batch::Batcher;

mod context;
pub use context::GlContext;

mod shader;
pub use shader::ShaderManager;

mod texture;
pub use texture::Texture;

pub mod particle;
pub mod text;

#[wasm_bindgen]
pub struct Renderer {
    #[wasm_bindgen(skip)]
    pub context: GlContext,
    #[wasm_bindgen(skip)]
    pub shader_manager: ShaderManager,
    #[wasm_bindgen(skip)]
    pub batcher: Batcher,
    #[wasm_bindgen(skip)]
    pub white_texture: Texture,
    #[wasm_bindgen(skip)]
    pub projection: [f32; 16],
}

impl Renderer {
    pub fn new(canvas_id: &str) -> Result<Self, JsValue> {
        let context = GlContext::new(canvas_id)?;
        let mut shader_manager = ShaderManager::new(&context);
        shader_manager.init_defaults(&context)?;

        context.gl.enable(web_sys::WebGl2RenderingContext::BLEND);
        context.gl.blend_func(
            web_sys::WebGl2RenderingContext::SRC_ALPHA,
            web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
        );

        let batcher = Batcher::new(&context)?;

        // Create and bind default white texture to unit 0
        let white_texture = Texture::create_white_pixel(&context)?;

        let mut renderer = Self {
            context,
            shader_manager,
            batcher,
            white_texture,
            projection: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        };
        // Upload initial projection
        renderer.set_projection(&[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);
        Ok(renderer)
    }

    pub fn clear(&self) {
        self.context.clear(0.1, 0.1, 0.1, 1.0);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.context.resize(width, height);
    }

    pub fn begin_frame(&mut self) {
        self.shader_manager.use_program(&self.context, "default");
        // Ensure u_texture is set to unit 0
        let loc = self
            .shader_manager
            .get_uniform_location(&self.context, "default", "u_texture");
        if let Some(loc) = loc {
            self.context.gl.uniform1i(Some(&loc), 0);
        }
    }

    pub fn set_projection(&mut self, matrix: &[f32]) {
        self.projection.copy_from_slice(matrix);
        self.shader_manager.use_program(&self.context, "default");
        self.shader_manager
            .set_uniform_matrix4fv(&self.context, "u_projection", matrix);
    }

    pub fn set_texture(&mut self, texture: &Texture) {
        self.batcher.set_texture(&self.context, texture);
    }

    pub fn draw_rect(
        &mut self,
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
        self.batcher.set_texture(&self.context, &self.white_texture);
        self.batcher
            .draw_rect(&self.context, x, y, w, h, r, g, b, a, model);
    }

    pub fn draw_texture_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u: f32,
        v: f32,
        uw: f32,
        vh: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        model: &[f32; 16],
    ) {
        self.batcher
            .draw_texture_rect(&self.context, x, y, w, h, u, v, uw, vh, r, g, b, a, model);
    }

    pub fn flush(&mut self) {
        self.batcher.flush(&self.context);
    }
}
