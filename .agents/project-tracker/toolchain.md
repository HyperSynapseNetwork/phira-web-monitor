---
sources:
  - ".github/workflows/*.yml"
  - "Cargo.toml"
  - "monitor-common/Cargo.toml"
  - "monitor-client/Cargo.toml"
  - "monitor-proxy/Cargo.toml"
  - "monitor-proxy/migration/Cargo.toml"
  - "README.md"
  - "web/package.json"
  - "web/vite.config.ts"
  - "web/.env"
---

# Toolchain & Dev Setup

## Build System

| Tool | Command | Output |
|------|---------|--------|
| Cargo workspace | `cargo build` | Rust crates for shared logic, proxy, migration binary, and WASM crate |
| wasm-pack | `cd monitor-client && wasm-pack build --out-dir ../web/pkg --target web` | Browser-consumable WASM package in `web/pkg/` |
| Vite | `cd web && npm run build` | Static frontend bundle in `web/dist/` |
| Cargo release build | `cargo build --release --bin monitor-proxy` | Production backend binary in `target/release/monitor-proxy` |

## Linting & Formatting

| Tool | Config file | Run command |
|------|-----------|-------------|
| `rustfmt` | No repo-local config found | `cargo fmt --all -- --check` |
| `clippy` | No repo-local config found | `cargo clippy --all-targets --all-features -- -D warnings` |
| TypeScript compiler | `web/tsconfig.json` | Indirectly exercised during `vite build` |

Notes:

- No ESLint, Prettier, or repo-local `rustfmt.toml` is checked in.
- Frontend quality gates are currently manual or build-based; CI does not run `npm` steps.

## Testing

| Aspect | Detail |
|--------|--------|
| Framework | Built-in Rust unit tests and async tests via `#[test]` / `#[tokio::test]` |
| Coverage target | Not specified in repo |
| Coverage tool | None checked in |
| E2E / integration | No dedicated browser or end-to-end suite found; parser and core logic are covered by Rust tests |

Representative coverage areas:

- `monitor-common/src/core/*` contains unit tests for BPM math, animation, chart metadata, object transforms, tween logic, and audio loading.
- `monitor-proxy/src/utils/parse/rpe.rs` contains async parser tests using `monitor-proxy/test_data/test_chart.json`.
- `monitor-proxy/src/error.rs` has a minimal conversion test.

## CI/CD Pipeline

GitHub Actions defines one workflow, `CI`, with three jobs:

1. `fmt` — checks formatting with `cargo fmt --all -- --check`.
2. `clippy` — runs `cargo clippy --all-targets --all-features -- -D warnings`.
3. `test` — runs `cargo test --all`.

There is no checked-in deploy job, frontend build job, or release publishing workflow.

## Development Environment

| Requirement | Value |
|-----------|-------|
| Required tools | `rustc`, `cargo`, `wasm-pack`, `npm` / Node.js, plus a reachable Phira API and mp server |
| Environment variables | `DATABASE_URL` is read by `monitor-proxy`; `VITE_API_BASE` is read by the frontend; README also documents `HSN_SECRET_KEY` for Phira/mp auth flows |
| Dev server / watcher | `cd web && npm run dev` starts Vite on port `3000`; `cargo run --bin monitor-proxy -- --debug` starts the backend on port `3080` by default |

Recommended local workflow:

1. Build `monitor-client` into `web/pkg/`.
2. Start the Vue dev server.
3. Start `monitor-proxy` with `--debug` and a development `DATABASE_URL`.
4. Point the frontend at the backend via `web/.env` or an environment-specific Vite env file.
