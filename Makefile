ID_CORE  := crates/id-core
WASM_OUT := ../../frontend/pkg

.PHONY: build build-release run wasm wasm-release clean

build: wasm
	cargo build

build-release: wasm-release
	cargo build --release

run: wasm
	cargo run -p server

wasm:
	wasm-pack build $(ID_CORE) --target web --out-dir $(WASM_OUT) --dev

wasm-release:
	wasm-pack build $(ID_CORE) --target web --out-dir $(WASM_OUT) --release

clean:
	cargo clean
	rm -rf frontend/pkg
