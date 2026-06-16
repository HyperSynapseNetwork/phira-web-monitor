---
sources:
  - "monitor-proxy/Cargo.toml"
  - "monitor-proxy/src/**/*.rs"
  - "monitor-proxy/migration/src/*.rs"
---

# Module: monitor-proxy

## Responsibility

`monitor-proxy` is the backend integration layer. It authenticates users against Phira, exposes browser-friendly HTTP/SSE/WebSocket endpoints, caches and parses charts, tracks visited users, and computes chart-ranking aggregates in the background.

## Entry Surface

- `src/main.rs` bootstraps config, DB, migrations, services, and the Axum server.
- `src/router.rs` defines public and protected routes, CORS behavior, and the static-file fallback.
- `src/services/*.rs` contains most of the business logic: auth, chart delivery, room monitoring, live monitor bridging, and chart ranking.
- `migration/src/*.rs` owns local schema evolution.

## Key Dependencies

- `axum`, `tokio`, `tower-http`, and `reqwest` for async server and upstream API work.
- `sea-orm` and `sea-orm-migration` for persistence.
- `jsonwebtoken` for proxy-issued JWTs.
- `zip`, `tempfile`, and parser utilities for chart archive processing.

## Notable Patterns

- Startup runs migrations before the server begins accepting traffic.
- `AppState` is the central dependency container and also owns the background ranking update task.
- `RoomService` and `LiveService` both speak to the Phira mp server but expose different browser-facing abstractions: SSE for room state, binary WebSocket events for gameplay.
- `ChartService` deliberately serializes parsed chart data into a compact binary format so the browser avoids archive parsing and YAML/JSON resource interpretation.
