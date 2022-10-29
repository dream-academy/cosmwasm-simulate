use cosmwasm_vm::{BackendApi, BackendError, BackendResult, GasInfo};
use bech32::{self, Variant, ToBase32};

//mock api
#[derive(Copy, Clone)]
pub struct RpcMockApi {
    canonical_length: usize,
}

impl RpcMockApi {
    pub fn new(canonical_length: usize) -> Self {
        RpcMockApi { canonical_length }
    }
}

const BECH32_PREFIX: &str = "wasm1";

impl BackendApi for RpcMockApi {
    fn canonical_address(&self, human: &str) -> BackendResult<Vec<u8>> {
        if !human.starts_with(BECH32_PREFIX) {
            return (
                Err(BackendError::user_err(
                    "Invalid input: human address does not begin with bech32 prefix",
                )),
                GasInfo::free(),
            );
        }
        let bech32_prefix_len = BECH32_PREFIX.len();
        let mut out = Vec::from(human)[bech32_prefix_len..].to_vec();
        let append = self.canonical_length - out.len();
        if append > 0 {
            out.extend(vec![0u8; append]);
        }
        (Ok(out), GasInfo::free())
    }

    fn human_address(&self, canonical: &[u8]) -> BackendResult<String> {
        if canonical.len() != self.canonical_length {
            return (
                Err(BackendError::user_err(
                    "Invalid input: canonical address length not correct",
                )),
                GasInfo::free(),
            );
        }

        // decode UTF-8 bytes into string
        if let Ok(human) = bech32::encode(BECH32_PREFIX, canonical.to_base32(), Variant::Bech32) {
            (Ok(human), GasInfo::free())
        } else {
            (
                Err(BackendError::user_err(
                    "Invalid input: canonical address not decodable",
                )),
                GasInfo::free(),
            )
        }
    }
}
