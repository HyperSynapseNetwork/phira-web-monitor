//! Phira Web Monitor - WASM Core
//!
//! This crate provides the core functionality for the Phira web-based
//! multiplayer game monitor, including chart rendering and event handling.

use wasm_bindgen::prelude::*;

/// A simple test function to verify WASM is working.
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Phira Web Monitor is ready.", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("World"), "Hello, World! Phira Web Monitor is ready.");
    }
}
