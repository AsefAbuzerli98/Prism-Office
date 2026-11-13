#!/usr/bin/env bash
# Build the DrawingML geometry + colour engine for the webview: crates/prism-drawingml → src/slide/wasm/.
# Requires: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.130
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p prism-drawingml --lib --target wasm32-unknown-unknown --no-default-features --features wasm --profile wasm
wasm-bindgen target/wasm32-unknown-unknown/wasm/prism_drawingml.wasm --target web --out-dir src/slide/wasm --out-name prism_drawingml --remove-name-section --remove-producers-section
if command -v wasm-opt >/dev/null; then wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int src/slide/wasm/prism_drawingml_bg.wasm -o src/slide/wasm/prism_drawingml_bg.wasm; fi
ls -la src/slide/wasm
