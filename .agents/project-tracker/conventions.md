---
sources:
  - "Cargo.toml"
  - "monitor-*/Cargo.toml"
  - "web/package.json"
  - "web/tsconfig.json"
  - ".github/workflows/*.yml"
  - "README.md"
---
# Project Conventions

> Agents MUST read and follow these conventions.

## Coding Conventions

| Aspect | Rule | Config |
|--------|------|--------|
| Rust formatter | Use `rustfmt`; CI enforces `cargo fmt --all -- --check`. | Rust default config |
| Rust linter | Clippy warnings are errors in CI. | `.github/workflows/ci.yml` |
| TypeScript style | Existing Vue files use double quotes, semicolons, Composition API, and two-space indentation. | `web/src/**/*.vue`, `web/src/**/*.ts` |
| Frontend build | Vite is the authoritative frontend build tool. | `web/package.json`, `web/vite.config.ts` |
| Max line length | Not explicitly configured. Match surrounding code and formatter output. | N/A |
| Trailing commas | Existing frontend code uses trailing commas in multiline literals and calls. Rust follows rustfmt. | Existing source style |

## Naming Conventions

| Category | Convention | Example |
|----------|------------|---------|
| Rust files / modules | `snake_case` modules grouped by responsibility. | `auth_handler.rs`, `chart_service.rs` |
| Rust variables / functions | `snake_case`. | `get_room_by_id`, `allowed_origin` |
| Rust types | `PascalCase`. | `AppState`, `ChartPlayer`, `RoomListResponse` |
| Vue components | `PascalCase` filenames. | `MonitorView.vue`, `PlayerView.vue` |
| TypeScript variables / functions | `camelCase`. | `wsBaseFromApi`, `selectedUserId` |
| Environment variables | `UPPER_SNAKE_CASE`. | `DATABASE_URL`, `VITE_API_BASE` |

## Architectural Rules

- Keep shared protocol and chart types in `monitor-common` when they are needed by both backend and WASM code.
- Keep Axum handlers thin; non-trivial backend behavior belongs in `monitor-proxy/src/services` or `monitor-proxy/src/utils`.
- Keep authenticated-only backend routes inside the protected router layer in `router.rs`.
- Preserve the WASM public API shape expected by `web/src/views`: `ChartPlayer` for standalone playback and `GameMonitor` for live monitoring.
- Build `monitor-client` into `web/pkg` before running or building the frontend because `web/package.json` depends on `monitor-client` as `file:./pkg`.
- Do not commit generated build outputs such as `target/`, `web/dist/`, `web/pkg/`, or `node_modules/` unless project policy changes.

## File Organization

| What | Where | Notes |
|------|-------|-------|
| Shared Rust domain types | `monitor-common/src/` | Chart, animation, audio, texture, and live protocol data. |
| Backend server | `monitor-proxy/src/` | `main.rs`, `router.rs`, handlers, services, middleware, entities, utilities. |
| Backend migrations | `monitor-proxy/migration/src/` | SeaORM migration crate. |
| WASM engine | `monitor-client/src/` | Browser-exposed classes plus renderer, engine, audio, and time modules. |
| Frontend UI | `web/src/` | Vue app, views, and i18n. |
| Frontend static assets | `web/public/assets/` | Default resource pack files. |
| CI | `.github/workflows/` | Rust-only CI at present. |

## Import / Module Conventions

- Rust modules are declared explicitly in `mod.rs`-style aggregator files or crate entry points.
- Public exports should be intentional; most backend modules are internal to `monitor-proxy`.
- Vue code uses ES module imports and Composition API inside `<script setup lang="ts">`.
- Avoid circular crate dependencies: `monitor-proxy` and `monitor-client` depend on `monitor-common`; `monitor-common` must remain independent from them.

## Error Handling

- Backend handlers return the project `Result<Response>` alias and convert service errors through `error` helpers.
- Use `?` for propagation and add context where errors cross subsystem boundaries.
- Startup uses `expect` for unrecoverable database migration and service initialization failures.
- WASM public methods return `Result<_, JsValue>` for JavaScript-visible failures.
- Log backend operational failures with `log` / `env_logger`; frontend/WASM currently logs browser-side failures to console.

## Testing Conventions

- Rust tests run with `cargo test --all`.
- Test coverage threshold is not configured in the repository.
- Prefer focused unit tests for parser/domain logic and integration-style tests for handlers/services where external services can be mocked or isolated.
- No checked-in frontend test runner exists; add one deliberately before depending on frontend automated tests.

## Documentation Conventions

- Keep README and tracker docs aligned when routes, build commands, or runtime configuration change.
- Use English for tracker docs even when README contains Chinese project documentation.
- Document public endpoints, CLI flags, environment variables, and generated-artifact prerequisites.

## Agent Instructions

- Use `rg` / `rg --files` for project search.
- Do not overwrite existing tracker files; create missing files and report skipped files.
- Verify code claims against source files rather than relying only on README text.
- After editing code or generated docs, run the relevant validation when feasible.
- Before commits, ensure no hardcoded secrets are introduced and run the Rust CI-equivalent checks when practical.
