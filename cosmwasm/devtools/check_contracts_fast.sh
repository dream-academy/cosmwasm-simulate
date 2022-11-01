#!/bin/bash
set -o errexit -o nounset -o pipefail
command -v shellcheck >/dev/null && shellcheck "$0"

for contract_dir in contracts/*/; do
  (
    cd "$contract_dir"
    cargo fmt
    mkdir -p target/wasm32-unknown-unknown/release/
    touch target/wasm32-unknown-unknown/release/"$(basename "$contract_dir" | tr - _)".wasm
    cargo check --tests
  )
done
