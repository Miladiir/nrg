#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

echo "=== 1/3  Checking for wasm-pack ==="
if ! command -v wasm-pack &> /dev/null; then
  echo "Installing wasm-pack..."
  curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

echo "=== 2/3  Building WebAssembly (id-core) ==="
wasm-pack build crates/id-core \
  --target web \
  --out-dir ../../frontend/pkg \
  --release

echo "=== 3/3  Building server ==="
cargo build --release -p server

echo ""
echo "Build complete!"
echo ""
echo "Run the server:"
echo "  ./target/release/server"
echo ""
echo "Then open:"
echo "  http://localhost:8080          (frontend)"
echo "  http://localhost:8080/swagger-ui (API docs)"
