# Stage 1: Build WebAssembly (id-core -> frontend/pkg)
FROM rust:slim@sha256:3b2879047d42784ca9403ad20c51ed3df361a50f1df96f5777d39b4e33aa65cd AS wasm-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /build
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates/ crates/

RUN wasm-pack build crates/id-core --target web --out-dir /build/frontend/pkg --release


# Stage 2: Build server binary
FROM rust:slim@sha256:3b2879047d42784ca9403ad20c51ed3df361a50f1df96f5777d39b4e33aa65cd AS server-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates/ crates/

RUN cargo build --release --bin server


# Stage 3: Runtime
FROM debian:bookworm-slim@sha256:60eac759739651111db372c07be67863818726f754804b8707c90979bda511df

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=server-builder /build/target/release/server ./server
COPY --from=wasm-builder   /build/frontend/pkg           ./frontend/pkg
COPY frontend/index.html                                  ./frontend/

EXPOSE 8080
CMD ["./server"]
