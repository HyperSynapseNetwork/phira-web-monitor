use wasm_bindgen::prelude::*;

mod audio;
mod chart_player;
mod engine;
pub mod game_monitor;
mod renderer;
pub mod time;

// For logging to JS console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => (crate::log(&format_args!($($t)*).to_string()))
}
