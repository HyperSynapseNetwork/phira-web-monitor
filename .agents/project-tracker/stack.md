---
sources:
  - "Cargo.toml"
  - "Cargo.lock"
  - "monitor-*/Cargo.toml"
  - "web/package.json"
  - "web/package-lock.json"
  - "README.md"
---
# Technology Stack

## Language & Runtime

| Property | Value |
|----------|-------|
| Backend language | Rust 2021 edition for `monitor-proxy`, `monitor-common`, and migration crate |
| WASM language | Rust 2024 edition for `monitor-client` |
| Frontend language | TypeScript 5.3 with Vue single-file components |
| Async runtime | Tokio 1 with full features |
| Browser runtime | WebAssembly, WebGL2, Web Audio, Fetch, and WebSocket APIs through `web-sys` |
| Package managers | Cargo and npm |

## Frameworks & Libraries

| Dependency | Version | Purpose |
|------------|---------|---------|
| `axum` | 0.8 | HTTP routing, JSON endpoints, SSE, and WebSocket upgrade handling in `monitor-proxy`. |
| `tokio` | 1 | Async execution for server networking, Phira multiplayer integration, and streaming work. |
| `tower-http` | 0.6 | CORS handling and static file serving for `web/dist`. |
| `reqwest` | 0.13 | Outbound calls to the configured Phira API base and streaming chart downloads. |
| `sea-orm` / `sea-orm-migration` | 1.1 | Database abstraction and startup migrations. |
| `jsonwebtoken` | 10.3 | JWT encoding/decoding for authenticated proxy sessions. |
| `wasm-bindgen` / `web-sys` | 0.2 / 0.3 | JavaScript bindings for the Rust browser engine. |
| `bincode` | 1.3 | Compact binary chart and live protocol serialization. |
| `symphonia` | 0.5.4 | Audio decoding support in shared chart/resource handling. |
| `nalgebra` | 0.32 | Math primitives for chart and rendering logic. |
| `Vue` | 3.5 | Frontend UI framework. |
| `Naive UI` | 2.43 | Frontend component library and dark theme primitives. |
| `vue-i18n` | 11.2 | English and Chinese localization. |
| `Vite` | 5 | Frontend dev server and static bundle generation. |
| `jszip` | 3.10 | Browser-side ZIP handling where needed by frontend workflows. |

## Database & Storage

| Component | Technology | Purpose |
|-----------|------------|---------|
| Primary DB | SQL database selected by `DATABASE_URL` | Stores visited Phira user IDs. SeaORM features enable SQLite, PostgreSQL, and MySQL backends. |
| ORM / migrations | SeaORM and SeaORM Migration | Entity mapping and startup schema migration. |
| Disk cache | Local filesystem under `--cache-dir` | Stores downloaded/processed chart resources and avoids repeated remote fetches. |
| Frontend assets | Static files under `web/public/assets` | Default resource pack images, sounds, and metadata. |

## Infrastructure & Services

- Phira API, configured by `--api-base`, is used for login/profile and chart retrieval.
- Phira multiplayer server, configured by `--mp-server`, is used for room state and live monitoring.
- GitHub Actions runs Rust formatting, Clippy, and test checks on push and pull request.
- Deployment is currently described through manual build commands and reverse proxy examples rather than checked-in Docker or Kubernetes assets.
