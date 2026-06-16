---
sources:
  - ".github/workflows/*.yml"
  - "Cargo.toml"
  - "monitor-common/Cargo.toml"
  - "monitor-client/Cargo.toml"
  - "monitor-proxy/Cargo.toml"
  - "monitor-common/src/**/*.rs"
  - "monitor-client/src/**/*.rs"
  - "monitor-proxy/src/**/*.rs"
  - "web/src/**/*"
  - "web/package.json"
  - "web/vite.config.ts"
---

# Project Conventions

> Agents MUST read and follow these conventions.

## Coding Conventions

| Aspect | Rule | Config |
|--------|------|--------|
| Formatter | Rust code is expected to satisfy `cargo fmt --all -- --check` | `.github/workflows/ci.yml` |
| Linter | Rust code is expected to satisfy `cargo clippy --all-targets --all-features -- -D warnings` | `.github/workflows/ci.yml` |
| Max line length | No explicit limit found; preserve existing formatting style | Not specified |
| Indentation | Rust, TypeScript, and Vue files use 4-space or 2-space indentation according to existing file style; preserve local style per file | Inferred from source |
| Quote style | TypeScript/Vue uses double quotes consistently; Rust uses standard Rust string style | Inferred from source |
| Semicolons | Required in Rust statements; TypeScript code uses semicolons | Inferred from source |
| Trailing commas | Common in Rust and TS multiline literals; preserve existing usage | Inferred from source |

## Naming Conventions

| Category | Convention | Example |
|----------|-----------|---------|
| Rust files / modules | `snake_case` | `topchart_service.rs`, `auth_middleware.rs` |
| Vue components | `PascalCase.vue` | `MonitorView.vue`, `PlayerView.vue` |
| Variables | Rust `snake_case`, TS `camelCase` | `cache_dir`, `activeTab` |
| Constants | `UPPER_SNAKE_CASE` | `MAX_PARALLEL_REQUESTS`, `QUERY_BUFFER` |
| Functions / methods | Rust `snake_case`, TS `camelCase` | `get_room_list`, `switchLocale` |
| Types / structs / enums | `PascalCase` | `AppState`, `ChartRankResponse`, `GameMonitor` |

## Architectural Rules

- Shared domain or protocol changes should land in `monitor-common` first, then be consumed by proxy and WASM code.
- Backend handlers should remain thin adapters over `services/*`; do not move business logic back into route handlers.
- Authentication for protected backend flows must go through `auth_middleware` and `AuthService`.
- Browser-facing chart payloads are serialized bincode outputs produced by the proxy; the frontend should not reimplement chart parsing in TypeScript.
- **Forbidden**: hand-edit generated package outputs in `web/pkg/`, `monitor-client/pkg/`, or built assets in `web/dist/` unless the change is explicitly about generated artifacts.

## File Organization

| What | Where | Notes |
|------|-------|-------|
| Shared source code | `monitor-common/src/` | Domain model and live protocol definitions |
| WASM client source | `monitor-client/src/` | Renderer, audio, player, and live-monitor runtime |
| Backend source | `monitor-proxy/src/` | Handlers, services, entities, middleware, utils |
| DB migrations | `monitor-proxy/migration/src/` | Incremental SeaORM migrations |
| Frontend source | `web/src/` | Vue views, app shell, and i18n |
| Tests | Co-located in Rust modules | No top-level `tests/` directory found |
| Static assets | `web/public/assets/` | Resource-pack images and sounds |
| Documentation | `README.md` and tracker docs | README is the only repo-native project doc found |

## Import / Module Conventions

- **Import style**: Rust code uses explicit `mod` trees and targeted `use` lists; TS/Vue code uses relative imports and local package imports such as `monitor-client`.
- **Module visibility**: modules are private by default and re-exported explicitly through `lib.rs`, `handlers.rs`, `services.rs`, `dtos.rs`, and `entity.rs`.
- **Circular dependencies**: not explicitly checked by tooling; keep the current layered dependency direction (`web` -> `monitor-client`; proxy/WASM -> `monitor-common`).

## Error Handling

- **Error representation**: backend routes use `crate::error::Result<T>` and `AppError`; service code commonly uses `anyhow::Result`.
- **Error propagation**: Rust uses `?` plus `AppErrorExt` helpers like `internal_server_error`, `bad_request`, and `unauthorized`.
- **Error context**: non-trivial service code wraps failures with context strings before surfacing them.
- **Panics / asserts**: acceptable at startup for unrecoverable setup failures (DB connect/migrate, service bootstrap) and in tests; avoid panics in request paths.

## Testing Conventions

- **Test location**: co-located next to Rust implementation modules under `#[cfg(test)]`.
- **Test naming**: descriptive Rust test names, usually one behavior per test.
- **Coverage target**: not specified.
- **Mocking strategy**: minimal mocking; parser and math logic are tested directly, often with real sample data from `monitor-proxy/test_data/`.

## Documentation Conventions

- **Doc comments**: present on some key modules and exported types, especially in WASM-facing code.
- **README**: currently the main project narrative, API reference, and deployment guide.
- **CHANGELOG**: no changelog file found.

## Agent Instructions

- No repo-local `AGENTS.md`, `.claude/CLAUDE.md`, `.claude/rules/`, or `.agents/rules/` files were found.
- Preserve the monorepo split between shared crate, backend, WASM crate, and frontend instead of collapsing responsibilities.
- Treat checked-in generated frontend/WASM artifacts as derived outputs unless the task explicitly asks to rebuild or update them.
- When documenting or changing runtime behavior, keep README claims aligned with actual routes, flags, and workspace member responsibilities.
