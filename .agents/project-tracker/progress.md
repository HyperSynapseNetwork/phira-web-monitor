# Progress & Roadmap

## Current Phase

Functional integration phase: the repository contains a working Rust backend, shared domain crate, WASM renderer, Vue frontend, CI checks, and deployment instructions, but frontend CI and operational packaging are not yet codified.

## Completed

- [x] Rust workspace with `monitor-common`, `monitor-proxy`, `monitor-client`, and migration crates.
- [x] Axum backend with public room/chart/visited endpoints, login endpoint, protected profile endpoint, and protected live WebSocket endpoint.
- [x] SeaORM migration for `visited_users` persistence.
- [x] Disk-based chart cache and server-side chart processing path.
- [x] WebAssembly chart player and live monitor engine with WebGL/Web Audio integration.
- [x] Vue 3 frontend with player and monitor tabs, Naive UI components, and English/Chinese i18n.
- [x] GitHub Actions CI for Rust formatting, Clippy, and tests.
- [x] README with architecture, API, development, and production deployment guidance.

## In Progress

- [ ] Stabilize production runtime configuration and secret/config documentation.
- [ ] Expand automated test coverage beyond current Rust checks.
- [ ] Align README runtime environment notes with `monitor-proxy::Config` requirements.

## Known Issues & Technical Debt

- Frontend build/test checks are not included in GitHub Actions.
- No Dockerfile, compose file, systemd unit, or deployment script is checked in.
- No health-check endpoint is defined for the backend.
- Coverage thresholds and coverage tooling are not configured.
- README mentions `HSN_SECRET_KEY`, while the current checked-in `Config` requires `DATABASE_URL` and does not define that variable.
- Some frontend console logging remains in runtime paths.

## Roadmap

- [ ] Add CI steps for `wasm-pack build`, `npm ci`, and `npm run build` after deciding how generated `web/pkg` should be managed in CI.
- [ ] Add backend health/readiness endpoints if the service is deployed behind orchestration or uptime monitoring.
- [ ] Add deployment artifacts such as Dockerfile, compose, or systemd service templates.
- [ ] Add integration tests for backend routes and service boundaries.
- [ ] Add frontend/component or browser tests for login, chart load, and live monitor flows.
- [?] Formalize API schema generation from Rust DTOs or a maintained OpenAPI document.
