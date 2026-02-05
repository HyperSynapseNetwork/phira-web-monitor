//! Phira Web Monitor - WASM Client
//!
//! This crate contains the WASM-specific logic, including:
//! - Data decoding (from proxy)
//! - WebGL rendering (TODO)
//! - Audio playback (TODO)

use monitor_common::{chart, rpe};
use wasm_bindgen::prelude::*;

// Initialize logging
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("error initializing logger");
}

/// A simple test function to verify WASM is working.
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Phira Web Monitor is ready.", name)
}

/// Parse an RPE chart from JSON string
/// Returns a JSON representation of parsing result
#[wasm_bindgen]
pub fn parse_chart(json: &str) -> Result<JsValue, JsValue> {
    match rpe::parse_rpe(json) {
        Ok(chart) => chart_to_json(&chart),
        Err(e) => Err(JsValue::from_str(&format!("Parse error: {}", e))),
    }
}

/// Decode a bincode-encoded chart from server
#[wasm_bindgen]
pub fn decode_chart(data: &[u8]) -> Result<JsValue, JsValue> {
    log::info!("Decoding {} bytes of chart data", data.len());
    match bincode::deserialize::<chart::Chart>(data) {
        Ok(chart) => chart_to_json(&chart),
        Err(e) => Err(JsValue::from_str(&format!("Decode error: {}", e))),
    }
}

fn chart_to_json(chart: &chart::Chart) -> Result<JsValue, JsValue> {
    let info = serde_json::json!({
        "success": true,
        "offset": chart.offset,
        "lineCount": chart.line_count(),
        "noteCount": chart.note_count(),
    });
    Ok(JsValue::from_str(&info.to_string()))
}
