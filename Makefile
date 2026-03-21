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

# ── wasm ──────────────────────────────────────────────────────────────────────
#
# Dev: call cargo + wasm-bindgen directly so we can pass --keep-debug.
#   wasm-pack strips DWARF sections via wasm-bindgen (no way to opt out),
#   which prevents Firefox from showing Rust source in the debugger.
#
# Release: wasm-pack is fine; we don't need debug info there.

WASM_TARGET := wasm32-unknown-unknown
WASM_RAW    := target/$(WASM_TARGET)/debug/id_core.wasm

wasm:
	cargo build -p id-core --target $(WASM_TARGET)
	wasm-bindgen $(WASM_RAW) \
	    --out-dir $(WASM_OUT) \
	    --target web \
	    --keep-debug

wasm-release:
	wasm-pack build $(ID_CORE) --target web --out-dir ../../$(WASM_OUT) --release

# ── housekeeping ──────────────────────────────────────────────────────────────

clean:
	cargo clean
	rm -rf $(WASM_OUT)
