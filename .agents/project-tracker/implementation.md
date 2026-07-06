---
sources:
  - "monitor-common/src/**/*.rs"
  - "monitor-client/src/**/*.rs"
  - "monitor-proxy/src/**/*.rs"
  - "monitor-proxy/migration/src/**/*.rs"
  - "monitor-proxy/test_data/*.json"
  - "web/src/**/*.ts"
  - "web/src/**/*.vue"
---
# Implementation Details

## Entry Points

| Target | File | Purpose |
|--------|------|---------|
| Backend binary | `monitor-proxy/src/main.rs` | Initializes logging, parses CLI/env config, connects database, runs migrations, initializes services, and starts Axum. |
| Backend router | `monitor-proxy/src/router.rs` | Registers public routes, protected routes, CORS, shared state, and static fallback serving. |
| Migration crate | `monitor-proxy/migration/src/lib.rs` | Exposes SeaORM `Migrator` used by backend startup. |
| WASM library | `monitor-client/src/lib.rs` | Defines browser-facing wasm module and exports chart/live modules. |
| Standalone player | `monitor-client/src/chart_player.rs` | Exposes `ChartPlayer` for chart fetch, decode, audio, autoplay, resize, and render loop. |
| Live monitor | `monitor-client/src/game_monitor.rs` | Exposes `GameMonitor` for live WebSocket event dispatch and per-player scene management. |
| Frontend app | `web/src/main.ts`, `web/src/App.vue` | Mounts Vue, i18n, Naive UI shell, and tabbed player/monitor views. |

## Key Algorithms & Logic

- Chart fetch path: `ChartPlayer::load_chart` requests `/chart/{id}`, receives bincode bytes, deserializes `(ChartInfo, Chart)`, sorts judge lines by z-index, restores resource packs, and updates renderer state.
- Chart rendering: `ChartPlayer::render` uses audio time as authoritative playback time, updates chart state, applies autoplay or miss logic, emits hit sounds and particles, renders, and flushes WebGL batches.
- Live monitor flow: `GameMonitor` opens a binary WebSocket, decodes `LiveEvent` frames into an event queue, sends `WsCommand` frames for join/leave/ready, and keeps per-user `GameScene` state.
- Backend startup: `AppState::new` connects to the configured database, applies migrations, builds service objects, and stores shared dependencies behind `Arc`.
- Routing: `router::init_router` separates public routes from routes wrapped by `require_auth` and applies CORS based on debug or explicit allowed origin.
- Persistence: SeaORM creates and accesses a single `visited_users` table keyed by `phira_id`.

## Error Handling Strategy

- Backend routes use a project-level `Result` and error conversion helpers to map failures into HTTP responses.
- Backend startup treats failed database connection, failed migration, or failed room service setup as fatal.
- WebSocket live handling logs decode and room command errors without panicking the whole server.
- WASM methods return `JsValue` errors to frontend callers and use browser console logging for diagnostics.
- Frontend views expose user-facing loading/error state for login and chart loading while keeping render-loop errors throttled.

## Testing Strategy

| Test level | Location | What it covers |
|------------|----------|----------------|
| Unit | Rust modules with `#[cfg(test)]` where present | Parser/domain/service behavior. |
| Workspace | `cargo test --all` | All Rust crates included in the workspace. |
| CI static checks | `.github/workflows/ci.yml` | Rust formatting and Clippy warnings as errors. |
| Test data | `monitor-proxy/test_data/test_chart.json` | Chart/parser fixture data. |
| Frontend | Not configured | No checked-in Vitest/Playwright/Cypress setup. |

## Performance Considerations

- Backend is async Tokio-based and streams chart responses rather than buffering all responses through handlers.
- Chart processing uses disk cache and in-flight request deduplication according to backend module documentation.
- WASM rendering batches WebGL work and uses a requestAnimationFrame loop from Vue views.
- `ChartPlayer` uses the audio engine clock as authoritative playback time to reduce audiovisual drift.
- Live monitor keeps headless scene state and attaches/detaches canvases, preserving state when views are hidden or players are selected.
