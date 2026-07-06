use wasm_bindgen::prelude::*;

mod audio;
mod chart_asset;
pub mod chart_player;
mod engine;
pub mod game_monitor;
mod renderer;
pub mod time;
mod viewport;

// For logging to JS console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => ($crate::log(&format_args!($($t)*).to_string()))
}
