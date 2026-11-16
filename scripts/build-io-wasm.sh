#!/usr/bin/env bash
# Build the document open/save engine (browser preview) for the webview: crates/prism-io → src/core/io-wasm/.
# Requires: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.130
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p prism-io --lib --target wasm32-unknown-unknown --no-default-features --features wasm --profile wasm
wasm-bindgen target/wasm32-unknown-unknown/wasm/prism_io.wasm --target web --out-dir src/core/io-wasm --out-name prism_io --remove-name-section --remove-producers-section
if command -v wasm-opt >/dev/null; then wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int src/core/io-wasm/prism_io_bg.wasm -o src/core/io-wasm/prism_io_bg.wasm; fi
ls -la src/core/io-wasm
