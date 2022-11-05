use prost_build::compile_protos;
use std::io::Result;

fn main() -> Result<()> {
    compile_protos(
        &[
            "proto/cosmos/bank/v1beta1/query.proto",
            "proto/cosmwasm/wasm/v1/query.proto",
            "proto/cosmwasm/wasm/v1/tx.proto",
        ],
        &["proto"],
    )?;
    Ok(())
}
