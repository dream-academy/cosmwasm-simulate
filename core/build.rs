use prost_build::compile_protos;
use std::env;
use std::io::Result;
use std::process::Command;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=../test-contract");
    compile_protos(
        &[
            "proto/cosmos/bank/v1beta1/query.proto",
            "proto/cosmwasm/wasm/v1/query.proto",
            "proto/cosmwasm/wasm/v1/tx.proto",
        ],
        &["proto"],
    )?;
    let _ = Command::new("cargo")
        .arg("wasm")
        .current_dir("../test-contract")
        .env("CARGO_TARGET_DIR", env::var_os("OUT_DIR").unwrap())
        .spawn()
        .expect("Failed to build test_contract");
    let _ = Command::new("cargo")
        .arg("+nightly")
        .arg("wasm")
        .current_dir("../test-contract-cov")
        .env("CARGO_TARGET_DIR", env::var_os("OUT_DIR").unwrap())
        .env(
            "RUSTFLAGS",
            "--emit=llvm-ir -C instrument-coverage -Zno-profiler-runtime",
        )
        .spawn()
        .expect("Failed to build test_contract_cov");
    Ok(())
}
