use crate::engine::{chart::ChartRenderer, resource::Resource};
use crate::renderer::Renderer;
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
pub struct Monitor {
    // Registry of views if we support multiple (optional for now)
    views: HashMap<i32, MonitorView>,
}

#[wasm_bindgen]
pub struct MonitorView {
    id: i32,
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
        Monitor {
            views: HashMap::new(),
        }
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
            id: user_id,
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
        self.renderer.set_projection(&[
            1.0, // x scale (maps [-1, 1] to [-1, 1])
            0.0, 0.0, 0.0, 0.0,
            -aspect, // y scale (maps [-1/aspect, 1/aspect] to [-1, 1], flipped)
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

        // Re-initialize resource and renderer
        // Note: In real app we might want to preserve some resources or reuse them
        let mut resource = Resource::new(self.renderer.context.width, self.renderer.context.height);
        resource.load_defaults(&self.renderer.context)?;
        resource.load_defaults(&self.renderer.context)?;
        self.chart_renderer = ChartRenderer::new(chart, resource);
        self.start_time = None;

        Ok(())
    }
}
