---
sources:
  - "Cargo.toml"
  - "monitor-common/src/**/*.rs"
  - "monitor-client/src/**/*.rs"
  - "monitor-proxy/src/**/*.rs"
  - "monitor-proxy/migration/src/*.rs"
  - "web/src/**/*"
  - "web/vite.config.ts"
---

# Architecture

## Overview

```
            +--------------------------------------+
            |         Phira HTTP API               |
            | auth / profile / chart / records     |
            +-------------------+------------------+
                                |
                                v
+-----------------+   HTTP/SSE/WS   +-----------------------------+
|   Vue 3 UI      | <--------------> |      monitor-proxy         |
|   web/          |                  | Axum + Tokio + SeaORM      |
| - auth/forms    |                  | - REST / SSE / WS bridge   |
| - monitor view  |                  | - chart cache + parsing    |
| - player view   |                  | - room monitor             |
+--------+--------+                  | - top-chart updater        |
         |                           +-------------+--------------+
         | imports local WASM                      |
         v                                         v
+-------------------------+           +----------------------------+
|   monitor-client        |           | Local DB + disk cache      |
| Rust -> WASM            |           | visited users / rankings   |
| - ChartPlayer           |           | serialized chart payloads  |
| - GameMonitor           |           +----------------------------+
| - WebGL + Audio         |
+------------+------------+
             ^
             |
     +-------+-------+
     | monitor-common|
     | shared chart  |
     | model + live  |
     | protocol      |
     +---------------+

Separate binary protocol connection:

monitor-proxy <----> Phira mp server (`mp_server`)
```

## Module Breakdown

| Module / Crate | Responsibility | Key types / exports |
|---------------|---------------|--------------------|
| `monitor-common` | Shared domain model for charts, assets, and live monitor protocol | `core::*`, `live::{WsCommand, LiveEvent}` |
| `monitor-client` | Browser-side WASM runtime for chart playback and multiplayer scene rendering | `ChartPlayer`, `GameMonitor`, renderer/audio/time modules |
| `monitor-proxy` | Backend proxy for auth, chart fetch/parse/cache, room state, live WS bridge, and chart rankings | `AppState`, `router::init_router`, handlers, services |
| `monitor-proxy/migration` | DB schema evolution | `Migrator`, migration files |
| `web` | Vue/TypeScript UI shell | `App.vue`, `PlayerView.vue`, `MonitorView.vue`, `i18n` |

## Data Flow

1. Frontend auth flow: `MonitorView.vue` sends `POST /auth/login`, stores the returned token, then calls `/auth/me` to hydrate profile state.
2. Room-list flow: the frontend and other consumers call `/rooms/info`, `/rooms/info/{id}`, `/rooms/user/{id}`, or subscribe to `/rooms/listen`; `RoomService` talks to the Phira mp server and replays cached events over SSE.
3. Live monitor flow: the frontend opens `/ws/live?token=...`; `auth_middleware` validates the JWT, `LiveService` authenticates against the mp server, and binary `LiveEvent`s are streamed to the WASM `GameMonitor`.
4. Chart playback flow: `PlayerView.vue` or `GameMonitor` requests `/chart/{id}`; `ChartService` either serves a cached serialized chart or downloads/unzips/parses it, then streams bincode output to the browser.
5. Ranking flow: `TopChartService` runs a background update task inside `AppState::new`, polls official chart/record endpoints, stores time-series counts in the DB, and exposes aggregate ranking endpoints.

## Design Patterns

- Shared-kernel crate: `monitor-common` centralizes binary protocol and chart-domain types so proxy and WASM client stay in lockstep.
- Service-oriented backend: handlers stay thin and delegate auth, chart, room, live, and ranking behavior to dedicated service modules.
- App-state dependency injection: Axum router state carries config, HTTP client, DB connection, and service singletons via `AppState`.
- Cache-aside chart delivery: chart binaries are fetched lazily, serialized once, and reused until the upstream `chartUpdated` marker changes.
- Event bridge: mp-server commands are translated into higher-level SSE or WebSocket events tailored for browser clients.

## Security Boundaries

- Protected routes: `/auth/me` and `/ws/live` require JWT validation through `auth_middleware`.
- Public routes: chart fetch, room lookup/listen, visited-user query, and ranking endpoints are intentionally unauthenticated in current code.
- CORS boundary: production mode requires `--allowed-origin`; debug mode mirrors request origins.
- Path safety: `ChartService::DirectoryLoader` normalizes extracted resource paths and strips unsafe path components before local reads.
- Startup boundary: database connectivity and schema migration are mandatory at boot; the process fails fast if they do not succeed.
