use super::context::GlContext;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlImageElement, WebGl2RenderingContext, WebGlTexture};

use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_TEXTURE_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Clone, Debug)]
pub struct Texture {
    pub texture: WebGlTexture,
    pub width: u32,
    pub height: u32,
    pub id: u32,
}

impl Texture {
    fn next_id() -> u32 {
        NEXT_TEXTURE_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub fn new(ctx: &GlContext) -> Result<Self, JsValue> {
        let texture = ctx.gl.create_texture().ok_or("failed to create texture")?;
        Ok(Self {
            texture,
            width: 0,
            height: 0,
            id: Self::next_id(),
        })
    }

    pub fn create_white_pixel(ctx: &GlContext) -> Result<Self, JsValue> {
        Self::create_solid_color(ctx, 1, 1, [255, 255, 255, 255])
    }

    pub fn create_solid_color(
        ctx: &GlContext,
        width: u32,
        height: u32,
        color: [u8; 4],
    ) -> Result<Self, JsValue> {
        let texture = ctx.gl.create_texture().ok_or("failed to create texture")?;
        ctx.gl
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));

        let size = (width * height) as usize;
        let mut pixels = Vec::with_capacity(size * 4);
        for _ in 0..size {
            pixels.extend_from_slice(&color);
        }

        ctx.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                WebGl2RenderingContext::TEXTURE_2D,
                0,
                WebGl2RenderingContext::RGBA as i32,
                width as i32,
                height as i32,
                0,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                Some(&pixels),
            )?;

        ctx.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );
        ctx.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );

        Ok(Self {
            texture,
            width,
            height,
            id: Self::next_id(),
        })
    }

    pub async fn load(ctx: &GlContext, url: &str) -> Result<Texture, JsValue> {
        let image = HtmlImageElement::new()?;
        image.set_cross_origin(Some("anonymous"));

        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            let on_load = Closure::once(move || {
                resolve.call0(&JsValue::NULL).unwrap();
            });
            let on_error = Closure::once(move |err| {
                reject.call1(&JsValue::NULL, &err).unwrap();
            });

            image.set_onload(Some(on_load.as_ref().unchecked_ref()));
            image.set_onerror(Some(on_error.as_ref().unchecked_ref()));

            on_load.forget();
            on_error.forget();
        });

        image.set_src(url);

        wasm_bindgen_futures::JsFuture::from(promise).await?;

        // Image loaded
        let texture = ctx.gl.create_texture().ok_or("failed to create texture")?;
        ctx.gl
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));

        // Use standard texImage2D with HtmlImageElement
        // Phira/Macroquad keeps V=0 at the Top.
        // Note: web-sys generates `tex_image_2d_with_u32_and_u32_and_html_image_element` for the overloaded signature
        // void texImage2D(GLenum target, GLint level, GLenum internalformat, GLenum format, GLenum type, HTMLImageElement? pixels);
        ctx.gl
            .tex_image_2d_with_u32_and_u32_and_html_image_element(
                WebGl2RenderingContext::TEXTURE_2D,
                0,
                WebGl2RenderingContext::RGBA as i32,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                &image,
            )?;

        ctx.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );
        ctx.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );
        ctx.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_S,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        ctx.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_T,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );

        ctx.gl.generate_mipmap(WebGl2RenderingContext::TEXTURE_2D);

        Ok(Texture {
            texture,
            width: image.width(),
            height: image.height(),
            id: Self::next_id(),
        })
    }

    pub async fn load_from_bytes(ctx: &GlContext, bytes: &[u8]) -> Result<Texture, JsValue> {
        let array = js_sys::Uint8Array::from(bytes);
        let blob_parts = js_sys::Array::new();
        blob_parts.push(&array);
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(
            &blob_parts,
            web_sys::BlobPropertyBag::new().type_("image/png"),
        )?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;

        let texture = Self::load(ctx, &url).await?;

        web_sys::Url::revoke_object_url(&url)?;

        Ok(texture)
    }
}
