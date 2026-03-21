WASM_OUT := frontend/pkg
ID_CORE   := crates/id-core

.PHONY: build build-release run wasm wasm-release clean

# ── default: debug ────────────────────────────────────────────────────────────

build: wasm
	cargo build

run: wasm
	cargo run -p server

# ── release ───────────────────────────────────────────────────────────────────

build-release: wasm-release
	cargo build --release

# ── wasm-pack ─────────────────────────────────────────────────────────────────

wasm:
	wasm-pack build $(ID_CORE) --target web --out-dir ../../$(WASM_OUT) --dev

wasm-release:
	wasm-pack build $(ID_CORE) --target web --out-dir ../../$(WASM_OUT) --release

# ── housekeeping ──────────────────────────────────────────────────────────────

clean:
	cargo clean
	rm -rf $(WASM_OUT)
