#!/bin/sh
# Builds the WebAssembly plugin and copies it next to lib.typ.
set -e
cd "$(dirname "$0")"
cargo build --release -p pele_typst_plugin --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/pele_typst_plugin.wasm pele.wasm
