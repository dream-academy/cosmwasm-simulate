use std::io::Result;
use tonic_build::configure;

fn main() -> Result<()> {
    let builder = configure().build_client(true);
    builder.compile(&["proto/cosmos/bank/v1beta1/query.proto"], &["proto"])?;
    Ok(())
}
