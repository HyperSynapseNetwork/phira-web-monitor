---
sources:
  - "monitor-common/src/**/*.rs"
  - "monitor-client/src/**/*.rs"
  - "monitor-proxy/src/**/*.rs"
  - "monitor-proxy/test_data/*"
  - "web/src/**/*"
---

# Implementation Details

## Entry Points

| Target | File | Purpose |
|--------|------|---------|
| Backend binary | `monitor-proxy/src/main.rs` | Parses config, opens the DB, runs migrations, constructs shared services, launches the ranking updater, and starts the Axum server |
| Migration binary | `monitor-proxy/migration/src/main.rs` | Runs SeaORM migrations for local persistence |
| WASM library | `monitor-client/src/lib.rs` | Exposes browser bindings and re-exports the player/monitor modules |
| Frontend app | `web/src/main.ts` | Boots Vue and i18n, then mounts `App.vue` |

## Key Algorithms & Logic

- Chart request path:
  `ChartService::handle_chart_request` looks up chart metadata, uses the upstream `chartUpdated` marker as a cache coherence key, serves a cached serialized chart if available, or downloads/unzips/parses the chart archive and writes a new cached bincode payload.

- Safe archive resource loading:
  `DirectoryLoader::load_file` rebuilds resource paths from normal path components only, preventing direct traversal through extracted archive references.

- Room-state fanout:
  `RoomMonitorState` stores the latest full room snapshot, appends incremental room events, and broadcasts them both as replayable SSE history and as live updates to subscribers.

- Live gameplay bridge:
  `LiveService` authenticates against the mp server, converts binary mp-server traffic into `monitor_common::live::LiveEvent`, and forwards it over a browser WebSocket where `GameMonitor` dispatches events to per-player scenes.

- Chart ranking updater:
  `TopChartService` periodically fetches all chart IDs, then fetches record counts in parallel with retry/backoff, stores raw counts in `chart_records`, computes rolling deltas for hour/day/week/month windows, and writes aggregate values into `chart_statistics`.

- WASM rendering split:
  `ChartPlayer` handles single-chart playback with audio-timed autoplay; `GameMonitor` manages multiple scenes, room events, deferred chart attachment, and per-user canvas lifecycle.

## Error Handling Strategy

- Backend route handlers return `Result<Response, AppError>` and keep HTTP-specific error mapping centralized in `error.rs`.
- Service layers use `anyhow` for richer context, then convert to HTTP-facing `AppError` near the handler boundary.
- Startup intentionally fails fast if DB connect/migrations or long-lived service bootstrap fails.
- Browser/WASM code mostly surfaces `JsValue` errors back to the UI layer; `MonitorView.vue` and `PlayerView.vue` translate those into status text or logs.

## Testing Strategy

| Test level | Location | What it covers |
|-----------|---------|---------------|
| Unit | `monitor-common/src/core/*.rs` | BPM/time math, animation, chart metadata behavior, object transforms, tweening, and audio loading |
| Unit / async | `monitor-proxy/src/utils/parse/rpe.rs`, `monitor-proxy/src/error.rs` | Parser correctness against sample chart data and basic error conversion |
| E2E | N/A | No dedicated browser or system-level E2E suite found |

## Performance Considerations

- The proxy streams chart binaries with `ReaderStream` instead of buffering the full response in memory after serialization.
- Disk caching avoids re-downloading and re-parsing unchanged charts.
- Ranking updates limit parallelism with `buffer_unordered(MAX_PARALLEL_REQUESTS)` and retry transient failures with exponential backoff.
- `ChartPlayer` uses the audio engine clock as the authoritative playback clock to keep rendering and sound aligned.
- The Vue shell keeps both major views mounted and uses `visibility:hidden` so WebGL state is preserved across tab switches.
