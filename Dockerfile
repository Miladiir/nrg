# Stage 1: Build WebAssembly (id-core -> frontend/pkg)
FROM rust:slim AS wasm-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /build
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates/ crates/

RUN wasm-pack build crates/id-core --target web --out-dir /build/frontend/pkg --release


# Stage 2: Build server binary
FROM rust:slim AS server-builder

WORKDIR /build
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates/ crates/

RUN cargo build --release --bin server


# Stage 3: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=server-builder /build/target/release/server ./server
COPY --from=wasm-builder   /build/frontend/pkg           ./frontend/pkg
COPY frontend/index.html                                  ./frontend/

EXPOSE 8080
CMD ["./server"]
