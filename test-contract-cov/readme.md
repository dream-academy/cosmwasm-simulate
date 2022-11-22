# Prerequisites

```bash
rustup +nightly target add wasm32-unknown-unknown
cargo install grcov
```

# Build Contract

Because **.wasm** files do not incorporate `llvm_cov_map` sections, we create a native object that acts as a holder for the coverage mapping instead. Because the block structure for the wasm object and the native object must be equal, we build the native object from the LLVM IR emitted during wasm build.

First, run `cargo wasm` and enable IR emission.

`RUSTFLAGS='--emit=llvm-ir -C instrument-coverage -Zno-profiler-runtime' cargo +nightly wasm`

Then, build the native object using IR.

`clang -c -o test-contract-cov.o target/wasm32-unknown-unknown/release/deps/test_contract_cov.ll`

After obtaining `cov.profraw` by running the contract, invoke `grcov` using the following command.

`grcov cov.profraw --branch -b artifacts/test_contract_cov.o -s . -t html -o cov_report`
