---
sources:
  - "Cargo.toml"
  - "Cargo.lock"
  - "monitor-*/Cargo.toml"
  - "web/package.json"
  - "web/package-lock.json"
  - "web/tsconfig.json"
  - "web/vite.config.ts"
  - ".github/workflows/*.yml"
  - "README.md"
---
# Toolchain & Dev Setup

## Build System

| Tool | Command | Output |
|------|---------|--------|
| Cargo | `cargo build --all` | Rust workspace binaries and libraries under `target/`. |
| Cargo release | `cargo build --release --bin monitor-proxy` | Optimized backend proxy binary. |
| wasm-pack | `cd monitor-client && wasm-pack build --target web --out-dir ../web/pkg --release` | WebAssembly package consumed by `web/package.json` as `monitor-client`. |
| npm / Vite | `cd web && npm run build` | Static frontend bundle in `web/dist`. |
| npm / Vite | `cd web && npm run dev` | Local frontend development server. |

## Linting & Formatting

| Tool | Config file | Run command |
|------|-------------|-------------|
| rustfmt | default Rust toolchain config | `cargo fmt --all -- --check` |
| Clippy | default Rust toolchain config | `cargo clippy --all-targets --all-features -- -D warnings` |
| TypeScript | `web/tsconfig.json` | No explicit npm script is defined; Vite build performs frontend bundling checks. |

## Testing

| Aspect | Detail |
|--------|--------|
| Framework | Rust built-in test harness through Cargo. |
| Command | `cargo test --all` |
| Coverage target | Not specified in project configuration. |
| Coverage tool | Not configured. |
| E2E / integration | No checked-in browser E2E framework. Backend includes `monitor-proxy/test_data/test_chart.json` for parser/chart-related testing inputs. |

## CI/CD Pipeline

GitHub Actions runs on every push and pull request for all branches:

1. `fmt`: installs stable Rust with `rustfmt` and runs `cargo fmt --all -- --check`.
2. `clippy`: installs stable Rust with `clippy`, restores Rust cache, and runs `cargo clippy --all-targets --all-features -- -D warnings`.
3. `test`: installs stable Rust, restores Rust cache, and runs `cargo test --all`.

There is no checked-in CI job for `wasm-pack`, npm install, TypeScript checking, or Vite build.

## Development Environment

| Requirement | Value |
|-------------|-------|
| Required tools | Rust stable toolchain, Cargo, Node.js 18 or newer, npm, wasm-pack. |
| Backend environment variables | `DATABASE_URL` is required by `monitor-proxy::Config`. Use a URL such as `sqlite://data.db?mode=rwc` for local SQLite. |
| Backend optional CLI flags | `--debug`, `--host`, `--port`, `--cache-dir`, `--api-base`, `--mp-server`, `--allowed-origin`. |
| Frontend environment variables | `VITE_API_BASE` optionally points the browser app at a separate backend origin; empty means same origin. |
| Dev backend | `DATABASE_URL='sqlite://data.db?mode=rwc' cargo run -p monitor-proxy -- --debug` |
| Dev frontend | Build `web/pkg` with wasm-pack first, then run `cd web && npm install && npm run dev`. |
