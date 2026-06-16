---
sources:
  - "Cargo.toml"
  - "monitor-common/Cargo.toml"
  - "monitor-client/Cargo.toml"
  - "monitor-proxy/Cargo.toml"
  - "monitor-proxy/migration/Cargo.toml"
  - "web/package.json"
  - "monitor-client/pkg/package.json"
---

# Technology Stack

## Language & Runtime

| Property | Value |
|----------|-------|
| Primary language | Rust |
| Rust editions | `monitor-common`, `monitor-proxy`, and `migration` use 2021; `monitor-client` uses 2024 |
| Browser language | TypeScript |
| Browser runtime | Vue 3 app in Vite; `monitor-client` compiled to WebAssembly |
| Package managers | `cargo` for Rust, `npm` for `web/`, `wasm-pack` for WASM packaging |

## Frameworks & Libraries

| Dependency | Version | Purpose |
|-----------|---------|---------|
| `axum` | `0.8` | HTTP server for REST, SSE, and WebSocket upgrade endpoints in `monitor-proxy` |
| `tokio` | `1.x` | Async runtime for server I/O, WebSocket handling, and background update loops |
| `tower-http` | `0.6` | CORS middleware and static file serving from `../web/dist` |
| `reqwest` | `0.13` | Proxy-side HTTP client for Phira API login, profile, chart, and ranking fetches |
| `sea-orm` | `1.1` | Persistence layer for visited users and chart ranking aggregates |
| `sea-orm-migration` | `1.1.0` | Incremental schema migrations for the local database |
| `jsonwebtoken` | `10.3` | Encodes and decodes session JWTs used by protected proxy endpoints |
| `zip` | `8.1` | Unpacks remote chart bundles before parsing and bincode serialization |
| `bincode` | `1.3` | Compact binary format used between proxy and WASM client for chart payloads |
| `wasm-bindgen` / `web-sys` | `0.2` / `0.3` | Browser bindings for WebAssembly rendering, networking, and audio |
| `vue` | `3.5.28` | Frontend UI shell around the WASM player and monitor views |
| `naive-ui` | `2.43.2` | Frontend component library for forms, cards, buttons, and layout widgets |
| `vue-i18n` | `11.2.8` | English and Simplified Chinese UI localization |
| `@vitejs/plugin-vue` + `vite` | `6.0.4` / `5.x` | Frontend build and dev server |
| `phira-mp-common` / `phira-mp-macros` | Git dependency | Shared multiplayer protocol types and binary serialization helpers |

## Database & Storage

| Component | Technology | Purpose |
|-----------|-----------|---------|
| Primary DB | SQL database via `DATABASE_URL` | Stores visited users plus chart-record/statistic tables |
| Default local DB | SQLite-style URL in docs (`sqlite://data.db?mode=rwc`) | Development default described by `Config` and README examples |
| ORM / Client | SeaORM | Query, insert, paginate, and migrate ranking/visited-user tables |
| Cache | Disk file cache under `~/.cache/hsn-phira` by default | Stores serialized chart binaries keyed by chart ID and update marker |
| File storage | Local filesystem and temporary directories | Downloads/unzips remote chart bundles before parsing |

## Infrastructure & Services

- Phira official HTTP API (`https://phira.5wyxi.com` by default) provides auth, chart metadata, chart archives, and chart-record counts.
- Phira multiplayer server (`localhost:12346` by default) provides room-state, live gameplay, and visit events over a separate binary protocol.
- GitHub Actions runs Rust formatting, clippy, and test checks on push and pull request.
- Static frontend hosting is expected to be handled by a generic web server such as Nginx or Caddy; no container or platform-specific deployment config is checked in.
