# Progress & Roadmap

## Current Phase

Feature expansion and stabilization of the proxy/runtime stack, with recent work adding chart-ranking persistence plus cache-related backend changes.

## Completed

- [x] Rust workspace split into shared core, backend proxy, migration crate, and WASM renderer/client.
- [x] Browser-side chart playback via `monitor-client` and the Vue `PlayerView`.
- [x] Authenticated multiplayer monitoring flow through `/auth/login`, `/auth/me`, `/ws/live`, and WASM `GameMonitor`.
- [x] Chart download, unzip, parse, bincode serialization, and disk-cache reuse in `ChartService`.
- [x] Room listing, room lookup, visited-user capture, and SSE room-event replay in `RoomService`.
- [x] GitHub Actions coverage for Rust formatting, clippy, and tests.
- [x] SeaORM migrations for `visited_users`, `chart_records`, and `chart_statistics`.
- [x] Ranking endpoints for hot charts and per-chart rank snapshots.

## In Progress

- [ ] Top-chart feature integration is present in the backend worktree, but no matching frontend surface was found under `web/src/`.
- [ ] Deployment remains manual; no checked-in container, process-manager, or platform automation is present.

## Known Issues & Technical Debt

- Frontend build/lint/test steps are not enforced in CI.
- Generated directories (`web/dist`, `web/pkg`, `monitor-client/pkg`) are checked into the repo, which raises the cost of keeping source and derived artifacts in sync.
- Persistence and deployment assumptions are lightly documented in code; the DB backend is configurable, but local docs/examples are SQLite-centric.
- Public API endpoints do not show explicit rate limiting or abuse controls in code.

## Roadmap

- [ ] Expose ranking endpoints in the frontend if the hot-chart feature is intended to be user-facing.
- [ ] Add frontend verification to CI, at minimum a `vite build` step and optionally type-checking.
- [ ] Document or automate the production process for web asset hosting plus backend process management.
- [?] Add broader integration coverage around proxy-to-mp-server interactions if a stable test harness becomes available.
