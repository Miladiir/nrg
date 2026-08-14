# Stage 1: Build WebAssembly (id-core -> frontend/pkg)
FROM rust:slim@sha256:6abf73f05806f36362d0ff2722f2250c6153398831edd0455e0e0baa1f78ecc7 AS wasm-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install --locked wasm-pack@0.15.0

WORKDIR /build
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY data/ data/

RUN wasm-pack build --target web --out-dir /build/frontend/pkg --release crates/id-core --features browser-wasm


# Stage 2: Build server binary
FROM rust:slim@sha256:6abf73f05806f36362d0ff2722f2250c6153398831edd0455e0e0baa1f78ecc7 AS server-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY data/ data/

RUN cargo build --release --bin server


# Stage 3: Runtime
FROM debian:bookworm-slim@sha256:60eac759739651111db372c07be67863818726f754804b8707c90979bda511df

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy all checked-in static assets first. `frontend/pkg` is excluded from the
# build context and then supplied by the reproducible WASM builder stage.
COPY --chown=65534:65534 frontend/ ./frontend/
COPY --from=server-builder --chown=65534:65534 /build/target/release/server ./server
COPY --from=wasm-builder --chown=65534:65534 /build/frontend/pkg ./frontend/pkg

RUN test -x ./server \
    && test -f ./frontend/index.html \
    && test -f ./frontend/app.css \
    && test -f ./frontend/app.js \
    && test -f ./frontend/swagger-ui/index.html \
    && test -f ./frontend/swagger-ui/app.js \
    && test -f ./frontend/pkg/id_core.js \
    && test -f ./frontend/pkg/id_core_bg.wasm

EXPOSE 8080
USER 65534:65534
CMD ["./server"]
