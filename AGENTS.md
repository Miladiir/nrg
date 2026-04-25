# AGENTS.md — nrg

## Run / Dev

- **Prerequisite:** `cargo install wasm-pack` (not in `Cargo.toml`)
- **Run locally:** `make run` — builds WASM dev build then starts server on `:8080`
- **Do NOT** rely on `cargo run -p server` alone on first run; the server serves `frontend/pkg/` which only exists after a wasm-pack build
- `cargo run -p server` is fine after the first `make run` or `make wasm`

## Workspace

- Two crates:
  - `crates/id-core` — library (`rlib` + `cdylib`), compiles to both native Rust and WASM
  - `crates/server` — Axum binary, depends on `id-core`
- Toolchain and targets managed in `rust-toolchain.toml` (`stable`, `wasm32-unknown-unknown`, musl targets)

## Build

- `make build` — dev WASM + Rust debug build
- `make build-release` — release WASM + Rust release build
- `make wasm` / `make wasm-release` — wasm-pack build to `frontend/pkg/`
- `make clean` — `cargo clean` + remove `frontend/pkg`

## Tests

- `cargo test -p id-core` (tests are inline in `crates/id-core/src/lib.rs`)
- No custom test harness or fixtures; standard `cargo test` works

## Lint / Format

- No custom config; use default `cargo clippy`, `cargo fmt`, `cargo check`

## Architecture Notes

- **Frontend:** Pure HTML/CSS/JS in `frontend/`, no bundler. Loads WASM module `frontend/pkg/id_core.js` directly.
- **WASM:** `id-core` exposes `wasm_*` functions gated behind `#[cfg(target_arch = "wasm32")]` and `#[wasm_bindgen]`.
- **Server:** Serves `frontend/` statically, Swagger UI at `/swagger-ui`, REST API under `/api`.
- **Docker:** Multi-stage build; static musl binary with `mimalloc`, scratch runtime. Multi-arch (`amd64`, `arm64`) via CI.

## CI / Ops

- Docker image built and pushed to `ghcr.io` on push/PR to `main` (see `.github/workflows/docker-build.yml`)
- Renovate runs daily for Cargo + Dockerfile + GitHub Actions deps (see `.github/renovate.json`)
- GitHub Actions versions are pinned with SHA comments for reproducibility
