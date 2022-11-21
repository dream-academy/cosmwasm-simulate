use std::fmt;

#[derive(Debug)]
pub enum Error {
    TokioError(String),
    RpcError(String),
    HttpError(String),
    InvalidArg(String),
    TendermintError(String),
    FormatError(String),
    VmError(String),
    StdError(String),
    IoError(String),
    BankError(String),
    BackendError(String),
}

impl Error {
    pub fn tokio_error<T: ToString>(msg: T) -> Self {
        Self::TokioError(msg.to_string())
    }

    pub fn rpc_error<T: ToString>(msg: T) -> Self {
        Self::RpcError(msg.to_string())
    }

    pub fn http_error<T: ToString>(msg: T) -> Self {
        Self::HttpError(msg.to_string())
    }

    pub fn invalid_argument<T: ToString>(msg: T) -> Self {
        Self::InvalidArg(msg.to_string())
    }

    pub fn tendermint_error<T: ToString>(msg: T) -> Self {
        Self::TendermintError(msg.to_string())
    }

    pub fn format_error<T: ToString>(msg: T) -> Self {
        Self::FormatError(msg.to_string())
    }

    pub fn vm_error<T: ToString>(msg: T) -> Self {
        Self::VmError(msg.to_string())
    }

    pub fn std_error<T: ToString>(msg: T) -> Self {
        Self::StdError(msg.to_string())
    }

    pub fn io_error<T: ToString>(msg: T) -> Self {
        Self::IoError(msg.to_string())
    }

    pub fn bank_error<T: ToString>(msg: T) -> Self {
        Self::BankError(msg.to_string())
    }

    pub fn backend_error<T: ToString>(msg: T) -> Self {
        Self::BackendError(msg.to_string())
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
            Self::HttpError(s) => {
                writeln!(f, "HTTP error: {}", s)?;
            }
            Self::InvalidArg(s) => {
                writeln!(f, "Invalid argument: {}", s)?;
            }
            Self::TendermintError(s) => {
                writeln!(f, "tendermint error: {}", s)?;
            }
            Self::FormatError(s) => {
                writeln!(f, "format error: {}", s)?;
            }
            Self::VmError(s) => {
                writeln!(f, "vm error: {}", s)?;
            }
            Self::StdError(s) => {
                writeln!(f, "std error: {}", s)?;
            }
            Self::IoError(s) => {
                writeln!(f, "I/O error: {}", s)?;
            }
            Self::BankError(s) => {
                writeln!(f, "bank error: {}", s)?;
            }
            Self::BackendError(s) => {
                writeln!(f, "backend error: {}", s)?;
            }
        }
        Ok(())
    }
}
