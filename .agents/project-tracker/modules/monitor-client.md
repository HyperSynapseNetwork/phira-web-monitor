---
sources:
  - "monitor-client/Cargo.toml"
  - "monitor-client/src/**/*.rs"
---

# Module: monitor-client

## Responsibility

`monitor-client` is the Rust `cdylib` compiled to WebAssembly. It turns shared chart data into playable or monitorable browser scenes using WebGL, Web Audio, and binary live events streamed from the proxy.

## Entry Surface

- `src/lib.rs` wires wasm-bindgen exports and browser console logging.
- `src/chart_player.rs` powers the standalone `/play` experience with chart loading, autoplay, and hitsound playback.
- `src/game_monitor.rs` powers the multiplayer monitor path with per-user scenes, live event queues, and on-demand canvas attachment.

## Key Dependencies

- `monitor-common` for chart types and live protocol types.
- `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys`, and `js-sys` for browser interop.
- `console_error_panic_hook` and `console_log` for runtime diagnostics in the browser.

## Notable Patterns

- Audio time is treated as authoritative during playback to keep visuals and sound synchronized.
- Scenes can exist headlessly before a canvas is attached, which lets the monitor view preserve room state independently from the DOM lifecycle.
- Resource packs are loaded from browser fetches and then passed into Rust as byte buffers.
