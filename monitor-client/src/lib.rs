use crate::engine::{chart::ChartRenderer, resource::Resource};
use monitor_common::core::Chart;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

mod audio;
mod engine;
mod network;
mod renderer;

// For logging to JS console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct Monitor {}

#[wasm_bindgen]
pub struct MonitorView {
    renderer: renderer::Renderer,
    chart_renderer: ChartRenderer,
    start_time: Option<f64>,
}

#[wasm_bindgen]
impl Monitor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Monitor {
        console_error_panic_hook::set_once();
        console_log!("Monitor Client Initialized");
        Monitor {}
    }

    pub fn monitor(&mut self, user_id: i32, canvas_id: String) -> Result<MonitorView, JsValue> {
        console_log!(
            "Creating MonitorView for User {} on Canvas '{}'",
            user_id,
            canvas_id
        );

        let renderer = renderer::Renderer::new(&canvas_id)?;

        // MVP: Initialize with dummy chart
        let mut resource = Resource::new(renderer.context.width, renderer.context.height);
        resource.load_defaults(&renderer.context)?;
        let mut chart = Chart::default();

        // Add a test JudgeLine
        use monitor_common::core::{Anim, AnimFloat, AnimVector, JudgeLine, JudgeLineKind, Object};
        let line = JudgeLine {
            object: Object {
                translation: AnimVector::default(),
                rotation: AnimFloat::default(),
                scale: AnimVector::default(),
                alpha: AnimFloat::fixed(1.0),
            },
            ctrl_obj: monitor_common::core::CtrlObject::default(),
            kind: JudgeLineKind::Normal,
            height: AnimFloat::default(),
            incline: AnimFloat::default(),
            notes: Vec::new(),
            color: Anim::default(),
            parent: None,
            z_index: 0,
            show_below: true,
            attach_ui: None,
        };

        chart.lines.push(line);

        let chart_renderer = ChartRenderer::new(chart, resource);

        Ok(MonitorView {
            renderer,
            chart_renderer,
            start_time: None,
        })
    }
}

#[wasm_bindgen]
impl MonitorView {
    pub fn render(&mut self) -> Result<(), JsValue> {
        self.renderer.clear();
        self.renderer.begin_frame();

        // Calculate and set orthographic projection (Width-normalized)
        // World width is 2.0 (-1.0 to 1.0)
        // World height is 2.0 / aspect
        // Y-axis is flipped (positive Y is down) to match Phira/RPE
        let aspect = self.chart_renderer.resource.aspect_ratio;
        // Use isotropic scaling (like Phira/prpr).
        // World width is 2.0 (-1.0 to 1.0)
        // World height is 2.0 / aspect (-1.0/aspect to 1.0/aspect)
        // NDC Y = World Y * aspect.
        let y_scale = aspect;

        self.renderer.set_projection(&[
            1.0, // x scale (maps [-1, 1] to [-1, 1])
            0.0, 0.0, 0.0, 0.0, y_scale, // y scale
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);

        let now = web_sys::window().unwrap().performance().unwrap().now();

        if self.start_time.is_none() {
            self.start_time = Some(now);
        }

        let time = (now - self.start_time.unwrap()) / 1000.0;
        self.chart_renderer.update(time as f32);

        self.chart_renderer.render(&mut self.renderer);
        self.renderer.flush();
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
        self.chart_renderer.resize(width, height);
    }

    pub async fn load_chart(&mut self, id: String) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let resp_value =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(&format!("/chart/{}", id)))
                .await?;
        let resp: web_sys::Response = resp_value.dyn_into()?;

        if !resp.ok() {
            return Err(JsValue::from_str(&format!(
                "Fetch failed: {}",
                resp.status_text()
            )));
        }

        let array_buffer = wasm_bindgen_futures::JsFuture::from(resp.array_buffer()?).await?;
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
        let vec = uint8_array.to_vec();

        use bincode::Options;
        let chart: monitor_common::core::Chart = bincode::options()
            .with_varint_encoding()
            .deserialize(&vec)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse chart: {}", e)))?;

        // Preserve existing resource pack if any
        let existing_pack = self.chart_renderer.resource.res_pack.take();

        // Re-initialize resource and renderer
        let renderer = &self.renderer; // Borrow renderer for context
        let mut resource = Resource::new(renderer.context.width, renderer.context.height);
        resource.load_defaults(&renderer.context)?;

        // Restore resource pack if we had one (and it wasn't just the default fallback, or even if it was)
        // Ideally we check if it's the fallback, but simply restoring the last active one is usually what we want
        // because loadResourcePack updates the active one.
        if let Some(pack) = existing_pack {
            if pack.info.name != "fallback" {
                resource
                    .set_pack(&renderer.context, pack)
                    .map_err(|e| JsValue::from_str(&format!("Failed to restore pack: {}", e)))?;
            }
        }

        // Load textures from chart
        use monitor_common::core::JudgeLineKind;
        for (i, line) in chart.lines.iter().enumerate() {
            if let JudgeLineKind::Texture(tex, _) = &line.kind {
                console_log!("Loading texture for line {}", i);
                match crate::renderer::Texture::load_from_bytes(&renderer.context, tex.data()).await
                {
                    Ok(texture) => {
                        resource.line_textures.insert(i, texture);
                    }
                    Err(e) => {
                        console_log!("Failed to load texture for line {}: {:?}", i, e);
                    }
                }
            }
        }

        // Also check if audio should be loaded (TODO)

        self.chart_renderer = ChartRenderer::new(chart, resource);
        self.start_time = None;

        Ok(())
    }

    pub async fn load_resource_pack(&mut self, files: js_sys::Object) -> Result<(), JsValue> {
        let entries = js_sys::Object::entries(&files);
        let mut file_map = HashMap::new();

        for i in 0..entries.length() {
            let entry = entries.get(i);
            let entry_array = js_sys::Array::from(&entry);
            let key = entry_array
                .get(0)
                .as_string()
                .ok_or("Invalid key in file map")?;
            let value = entry_array.get(1);
            let uint8_array = js_sys::Uint8Array::new(&value);
            let bytes = uint8_array.to_vec();
            file_map.insert(key, bytes);
        }

        console_log!("Loading resource pack with {} files", file_map.len());

        use crate::engine::resource::ResourcePack;
        let res_pack = ResourcePack::load(&self.renderer.context, file_map)
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to load resource pack: {:?}", e)))?;

        console_log!("Resource pack loaded: {}", res_pack.info.name);

        self.chart_renderer
            .resource
            .set_pack(&self.renderer.context, res_pack)
            .map_err(|e| JsValue::from_str(&format!("Failed to set resource pack: {}", e)))?;

        Ok(())
    }
}
