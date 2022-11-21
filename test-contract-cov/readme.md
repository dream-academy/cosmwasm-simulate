# Prerequisites

`rustup +nightly target add wasm32-unknown-unknown`

# Build

`RUSTFLAGS='-C instrument-coverage -Zno-profiler-runtime' cargo +nightly wasm`
