use std::io::Result;
use prost_build::compile_protos;

fn main() -> Result<()> {
    compile_protos(&["proto/cosmos/bank/v1beta1/query.proto"], &["proto"])?;
    Ok(())
}
