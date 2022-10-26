use std::fmt;

#[derive(Debug)]
pub enum Error {
    TokioError(String),
    RpcError(String),
    InvalidArg(String),
    TendermintError(String),
    ProtobufError(String),
}

impl Error {
    pub fn tokio_error<T: ToString>(msg: T) -> Self {
        Self::TokioError(msg.to_string())
    }

    pub fn rpc_error<T: ToString>(msg: T) -> Self {
        Self::RpcError(msg.to_string())
    }

    pub fn invalid_argument<T: ToString>(msg: T) -> Self {
        Self::InvalidArg(msg.to_string())
    }

    pub fn tendermint_error<T: ToString>(msg: T) -> Self {
        Self::TendermintError(msg.to_string())
    }

    pub fn protobuf_error<T: ToString>(msg: T) -> Self {
        Self::ProtobufError(msg.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokioError(s) => {
                writeln!(f, "tokio error: {}", s)?;
            }
            Self::RpcError(s) => {
                writeln!(f, "RPC error: {}", s)?;
            }
            Self::InvalidArg(s) => {
                writeln!(f, "Invalid argument: {}", s)?;
            }
            Self::TendermintError(s) => {
                writeln!(f, "tendermint error: {}", s)?;
            }
            Self::ProtobufError(s) => {
                writeln!(f, "protobuf error: {}", s)?;
            }
        }
        Ok(())
    }
}
