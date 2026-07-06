---
sources:
  - "README.md"
  - "Cargo.toml"
  - "monitor-*/Cargo.toml"
  - "web/package.json"
  - ".github/workflows/*.yml"
---
# PROJECT: HSN Phira Web Monitor

HSN Phira Web Monitor is a Rust, WebAssembly, and Vue toolchain for proxying Phira multiplayer room data and rendering live or standalone chart playback in the browser.

## Table of Contents

- [Stack](stack.md) - Technology choices and dependencies
- [Toolchain](toolchain.md) - Build, test, CI/CD, and dev setup
- [Architecture](architecture.md) - Module layout and data flow
- [Conventions](conventions.md) - Coding standards, naming rules, and project rules
- [Progress](progress.md) - Current status and roadmap
- [Implementation](implementation.md) - Entry points and key implementation details
- [Data Model](data-model.md) - Database, entity, migration, and cache model
- [API](api.md) - HTTP, SSE, and WebSocket API surface
- [Deployment](deployment.md) - Build artifacts and deployment notes

## Tech Stack Summary

| Layer | Technology | Version |
|-------|------------|---------|
| Backend | Rust workspace, Tokio, Axum | Rust 2021 crates, Axum 0.8 |
| Browser engine | Rust WebAssembly via wasm-bindgen | Rust 2024 crate, wasm-bindgen 0.2 |
| Frontend | Vue 3, TypeScript, Vite, Naive UI | Vue 3.5, TypeScript 5.3, Vite 5 |
| Database | SeaORM over SQLx backends | SeaORM 1.1 |
| CI/CD | GitHub Actions | format, clippy, test |

- `monitor-proxy` is the Axum server that authenticates users, proxies Phira data, streams room events, serves chart binaries, and optionally serves `web/dist`.
- `monitor-client` is the WebAssembly rendering core that exposes `ChartPlayer` and `GameMonitor` to the Vue app.
- `monitor-common` contains shared chart, audio, texture, animation, and live protocol types.
- `web` is a Vue single-page frontend with separate player and monitor views.

## Quick Reference Commands

```bash
# Build Rust workspace
cargo build --all

# Test Rust workspace
cargo test --all

# Lint and format check
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Build WASM client for the web app
cd monitor-client
wasm-pack build --target web --out-dir ../web/pkg --release

# Run frontend dev server
cd web
npm install
npm run dev

# Run backend proxy
DATABASE_URL='sqlite://data.db?mode=rwc' cargo run -p monitor-proxy -- --debug
```

## Project Map

- `monitor-common/` - shared Rust data structures and parsing/rendering domain types.
- `monitor-proxy/` - Axum backend, SeaORM migration, handlers, services, middleware, and test data.
- `monitor-client/` - WebAssembly rendering, audio, chart player, and live game monitor engine.
- `web/` - Vue 3 frontend, i18n resources, static resource pack assets, and Vite config.
- `.github/workflows/` - GitHub Actions CI for Rust format, lint, and test checks.

## Tracking Exclusions

- `target/**` - Cargo build output.
- `web/dist/**` - generated frontend bundle.
- `web/pkg/**` - generated wasm-pack output consumed by the frontend.
- `node_modules/**` - npm dependency installation output.
