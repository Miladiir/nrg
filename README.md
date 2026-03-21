# nrg

Generate and validate German energy market location IDs (MaLo-ID, MeLo-ID, NeLo-ID).

## Prerequisites

- [Rust](https://rustup.rs) (toolchain managed via `rust-toolchain.toml`)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/): `cargo install wasm-pack`

## Run

```bash
cargo run -p server
```

On first run, `build.rs` compiles `id-core` to WebAssembly via wasm-pack before starting the server.

## Endpoints

- `http://localhost:8080` — frontend
- `http://localhost:8080/swagger-ui` — API docs
