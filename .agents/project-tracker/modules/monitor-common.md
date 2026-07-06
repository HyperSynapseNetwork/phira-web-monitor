---
sources:
  - "monitor-common/Cargo.toml"
  - "monitor-common/src/**/*.rs"
---

# Module: monitor-common

## Responsibility

`monitor-common` is the shared kernel crate. It owns the chart-domain model, rendering math primitives, audio/chart metadata types, and the live WebSocket protocol types used by both `monitor-proxy` and `monitor-client`.

## Entry Surface

- `src/lib.rs` re-exports `core` and `live`.
- `src/core.rs` re-exports the chart/rendering domain model from submodules such as `chart`, `anim`, `bpm`, `object`, `texture`, and `audio`.
- `src/live.rs` defines browser-facing `WsCommand` and `LiveEvent` enums using the same binary-data derive path as the mp protocol.

## Key Dependencies

- `phira-mp-common` and `phira-mp-macros` for multiplayer protocol compatibility.
- `serde`, `serde_json`, `serde_yaml`, and `bincode` for serialization formats.
- `nalgebra`, `half`, `image`, `chrono`, and `symphonia` for math/media support.

## Notable Patterns

- Re-exporting `BinaryData`, `BinaryReader`, `BinaryWriter`, and `Result` at the crate root is intentional because the derive macros generate `crate::...` references.
- Domain logic is test-heavy relative to the rest of the repo; most core modules include focused unit coverage.
