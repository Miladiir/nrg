ID_CORE  := crates/id-core
WASM_OUT := ../../frontend/pkg
WORKER   := crates/cloudflare-worker

.PHONY: build build-release run wasm wasm-release worker cloudflare clean

build: wasm
	cargo build --workspace --exclude cloudflare-worker

build-release: wasm-release
	cargo build --release --workspace --exclude cloudflare-worker

run: wasm
	cargo run -p server

wasm:
	wasm-pack build --mode no-install --target web --out-dir $(WASM_OUT) --dev $(ID_CORE) --features browser-wasm

wasm-release:
	wasm-pack build --mode no-install --target web --out-dir $(WASM_OUT) --release $(ID_CORE) --features browser-wasm

worker:
	worker-build $(WORKER) --release --no-panic-recovery

cloudflare: wasm-release worker

clean:
	cargo clean
	rm -rf frontend/pkg
	rm -rf crates/cloudflare-worker/build
