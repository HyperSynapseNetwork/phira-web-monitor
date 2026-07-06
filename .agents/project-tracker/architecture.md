---
sources:
  - "README.md"
  - "Cargo.toml"
  - "monitor-*/Cargo.toml"
  - "monitor-common/src/**/*.rs"
  - "monitor-client/src/**/*.rs"
  - "monitor-proxy/src/**/*.rs"
  - "monitor-proxy/migration/src/**/*.rs"
  - "web/src/**/*.ts"
  - "web/src/**/*.vue"
---
# Architecture

## Overview

```text
+-----------------------+        HTTP/SSE/WS        +-----------------------+
| Vue 3 browser app     | <-----------------------> | monitor-proxy         |
| web/src               |                           | Axum + Tokio          |
+----------+------------+                           +----------+------------+
           |                                                   |
           | wasm-bindgen                                      | reqwest / phira-mp
           v                                                   v
+-----------------------+                         +-------------------------+
| monitor-client WASM   |                         | External Phira services |
| WebGL2 + Web Audio    |                         | API + multiplayer       |
+----------+------------+                         +-------------------------+
           ^                                                   |
           | shared chart/live types                           | SeaORM
           |                                                   v
+----------+------------+                         +-------------------------+
| monitor-common        |                         | SQL database + disk     |
| chart/live domain     |                         | chart cache             |
+-----------------------+                         +-------------------------+
```

The backend bridges browser clients to Phira services. The WASM client renders chart playback and live monitored scenes. Shared Rust data structures keep the backend, live protocol, and browser engine aligned.

## Module Breakdown

| Module / Crate | Responsibility | Key types / exports |
|----------------|----------------|---------------------|
| `monitor-common` | Shared domain model for chart, animation, audio, texture, and live protocol data. | `core::*`, `live::*` |
| `monitor-client` | WebAssembly rendering/audio engine and browser-facing API. | `ChartPlayer`, `GameMonitor`, `GameScene`, renderer and engine modules |
| `monitor-proxy` | Axum server, route registration, auth middleware, Phira service adapters, chart cache, and database access. | `AppState`, `Config`, `router::init_router`, handlers, services |
| `monitor-proxy/migration` | SeaORM migrations for backend persistence. | `Migrator`, `m20220101_000001_create_table` |
| `web` | Vue UI for standalone playback and live monitoring. | `App.vue`, `PlayerView.vue`, `MonitorView.vue`, i18n locales |

## Data Flow

1. Standalone playback: user enters a chart ID in `PlayerView.vue`; `ChartPlayer` fetches `/chart/{id}`; `monitor-proxy` downloads/parses/caches the chart and streams bincode bytes; WASM deserializes and renders it with WebGL and Web Audio.
2. Live monitoring: user logs in through `/auth/login`; the frontend stores the returned token, loads `/auth/me`, opens `/ws/live?token=...`, and sends binary `WsCommand` frames for room actions.
3. Room discovery: browser or consumers call `/rooms/info`, `/rooms/info/{id}`, `/rooms/user/{id}`, or `/rooms/listen`; `RoomService` talks to the Phira multiplayer server and emits JSON/SSE events.
4. Persistence: startup connects to `DATABASE_URL`, runs migrations, and stores visited Phira IDs in `visited_users`.

## Design Patterns

- Shared kernel crate: `monitor-common` centralizes protocol and chart types to avoid duplicated frontend/backend schemas.
- Service layer: `monitor-proxy/src/services` separates Phira auth, chart, room, and live operations from Axum handlers.
- Application state container: `AppState` wraps shared config, database, HTTP client, and services in `Arc` for handler cloning.
- Binary protocol boundary: live WebSocket messages use `phira-mp-common` packet encoding/decoding and shared `monitor-common::live` commands/events.
- Browser facade: WASM exposes small JS-facing classes while keeping rendering/audio details in Rust modules.

## Security Boundaries

- `/auth/me` and `/ws/live` are protected by `middlewares::require_auth`; other routes are public.
- Production CORS requires `--allowed-origin`; `--debug` mirrors origins for local development.
- Login forwards credentials to the configured Phira API and returns a proxy JWT token.
- `DATABASE_URL`, external API base, multiplayer server address, and allowed origin are runtime configuration boundaries.
- Chart IDs and room IDs are route inputs; room IDs are converted through `RoomId::try_from` and invalid values become bad requests.
