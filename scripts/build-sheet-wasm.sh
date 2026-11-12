#!/usr/bin/env bash
# Build the spreadsheet engine for the webview: crates/prism-sheet → src/sheet/wasm/.
# Requires: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.130
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p prism-sheet --lib --target wasm32-unknown-unknown --no-default-features --features wasm --profile wasm
wasm-bindgen target/wasm32-unknown-unknown/wasm/prism_sheet.wasm --target web --out-dir src/sheet/wasm --out-name prism_sheet --remove-name-section --remove-producers-section
if command -v wasm-opt >/dev/null; then wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int src/sheet/wasm/prism_sheet_bg.wasm -o src/sheet/wasm/prism_sheet_bg.wasm; fi
ls -la src/sheet/wasm
