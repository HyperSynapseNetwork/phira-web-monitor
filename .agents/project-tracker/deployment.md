---
sources:
  - "README.md"
  - "monitor-proxy/src/config.rs"
  - "monitor-proxy/src/main.rs"
  - "monitor-proxy/src/router.rs"
  - "web/package.json"
  - "web/vite.config.ts"
---

# Deployment

## Build Artifacts

| Artifact | Format | How to build |
|----------|--------|-------------|
| `web/pkg/*` | WASM package | `cd monitor-client && wasm-pack build --target web --out-dir ../web/pkg --release` |
| `web/dist/*` | Static SPA bundle | `cd web && npm ci && npm run build` |
| `target/release/monitor-proxy` | Native Rust binary | `cargo build --release --bin monitor-proxy` |

## Packaging

The repo ships a manual packaging flow rather than a checked-in container or release manifest:

1. Build `monitor-client` into `web/pkg`.
2. Build the Vue app into `web/dist`.
3. Build the backend binary.
4. Host `web/dist` behind a static web server and reverse-proxy API/SSE/WebSocket traffic to `monitor-proxy`.

No `Dockerfile`, `docker-compose.yml`, Kubernetes manifests, or platform-specific deployment scripts were found.

## Environments

| Environment | URL / target | Promotion from | Notes |
|------------|-------------|---------------|-------|
| Local dev | `http://localhost:3000` frontend, `http://localhost:3080` backend | -- | Vite dev server plus debug backend |
| Manual production | Operator-defined domain | Local build outputs | README shows an Nginx-style reverse-proxy deployment |
| Staging | N/A | N/A | No dedicated staging config found |

## Health Checks

| Check | Endpoint / command | Expected |
|-------|-------------------|----------|
| Backend process start | `./target/release/monitor-proxy --port <PORT> --allowed-origin <ORIGIN>` | Process binds successfully and serves routes |
| Frontend asset build | `cd web && npm run build` | `web/dist/index.html` and asset bundle are emitted |
| Rust verification | `cargo test --all` | Test suite passes |

No dedicated `/health` or `/ready` endpoints were found in the current router.

## Monitoring & Alerts

N/A — no checked-in production monitoring, metrics, or alerting configuration was found. The backend logs through `env_logger`, so operational visibility currently depends on the host process manager and log aggregation outside this repo.

## Rollback Procedure

1. Keep the previous `monitor-proxy` binary and prior `web/dist` bundle available on the host.
2. If a deploy fails, stop the current backend process and restart the previous binary.
3. Restore the previous `web/dist` bundle in the static host directory.
4. Confirm that REST, SSE, and `/ws/live` traffic recover behind the reverse proxy.

This is an inferred manual rollback path; no automated rollback tooling is checked in.
