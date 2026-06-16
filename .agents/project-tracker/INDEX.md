---
sources:
  - "Cargo.toml"
  - "README.md"
  - "monitor-common/Cargo.toml"
  - "monitor-client/Cargo.toml"
  - "monitor-proxy/Cargo.toml"
  - "web/package.json"
  - ".github/workflows/*.yml"
---

# PROJECT: HSN Phira Web Monitor

A Rust-and-WASM toolchain for watching Phira multiplayer rooms in the browser, proxying official APIs, and rendering charts live in WebGL.

## Table of Contents

- [Stack](stack.md) — Technology choices and dependencies
- [Toolchain](toolchain.md) — Build, test, CI/CD, dev setup
- [Architecture](architecture.md) — Workspace layout and runtime data flow
- [Conventions](conventions.md) — Coding standards, naming rules, architectural rules
- [Progress](progress.md) — Current status and roadmap
- [Implementation](implementation.md) — Key implementation details
- [Data Model](data-model.md) — Persistence model and cache ownership
- [API](api.md) — REST, SSE, and WebSocket surfaces
- [Deployment](deployment.md) — Build artifacts and manual deployment flow
- [Modules](modules/monitor-common.md) — Shared parsing and protocol types
- [Modules](modules/monitor-client.md) — WASM renderer and monitor runtime
- [Modules](modules/monitor-proxy.md) — Axum proxy, services, and DB-backed ranking
- [Modules](modules/web.md) — Vue 3 shell around the WASM client

## Tech Stack Summary

| Layer | Technology | Version |
|-------|-----------|---------|
| Backend language | Rust | Workspace uses 2021 and 2024 editions |
| Backend framework | Axum + Tokio + SeaORM | `axum 0.8`, `tokio 1`, `sea-orm 1.1` |
| Frontend | Vue 3 + TypeScript + Vite + Naive UI | Vue `3.5.x`, Vite `5.x` |
| Binary protocols | `phira-mp-common`, `bincode`, WASM bindings | Git dependency + `bincode 1.3` |
| CI/CD | GitHub Actions | Rust fmt, clippy, test |

- Rust owns the protocol, parsing, caching, proxy, and rendering core.
- `monitor-client` is compiled to WebAssembly and consumed locally by the Vue app.
- `monitor-proxy` exposes REST, SSE, and authenticated WebSocket endpoints.
- A small SeaORM migration crate owns local persistence for visited users and chart-rank aggregates.

## Quick Reference Commands

```bash
# Build the Rust workspace
cargo build

# Build the WASM package used by the web app
cd monitor-client && wasm-pack build --out-dir ../web/pkg --target web

# Run Rust tests
cargo test --all

# Run the backend locally
DATABASE_URL=sqlite://data.db?mode=rwc cargo run --bin monitor-proxy -- --debug

# Run the frontend locally
cd web && npm install && npm run dev

# Lint / checks
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Project Map

- `monitor-common/` — shared chart model, parsing helpers, and live protocol types used by proxy and WASM client.
- `monitor-proxy/` — Axum server, SeaORM entities/migrations, API handlers, SSE/WebSocket bridge, and chart/top-rank services.
- `monitor-client/` — Rust `cdylib` compiled to WASM for chart playback and multiplayer scene monitoring.
- `web/` — Vue 3 + TypeScript UI that authenticates, drives WebSocket/SSE flows, and hosts canvases for WASM rendering.

## Tracking Exclusions

- `web/node_modules/**` — installed dependencies, not maintained by hand.
- `web/dist/**` — built frontend artifacts.
- `web/pkg/**` — generated WASM package consumed by the frontend.
- `monitor-client/pkg/**` — generated local package output.
- `data.db` — runtime database artifact, not a source input.
