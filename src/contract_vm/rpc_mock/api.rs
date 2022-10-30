use bech32::{self, FromBase32, ToBase32, Variant};
use cosmwasm_vm::{BackendApi, BackendError, BackendResult, GasInfo};

use crate::contract_vm::Error;

const BECH32_PREFIX_MAX_LEN: usize = 10;

//mock api
#[derive(Copy, Clone)]
pub struct RpcMockApi {
    canonical_length: usize,
    bech32_prefix: [u8; BECH32_PREFIX_MAX_LEN],
    bech32_prefix_len: usize,
}

impl RpcMockApi {
    pub fn new(canonical_length: usize, bech32_prefix_str: &str) -> Result<Self, Error> {
        let bech32_prefix_len = bech32_prefix_str.len();
        if bech32_prefix_len > BECH32_PREFIX_MAX_LEN {
            Err(Error::invalid_argument(&format!(
                "bech32 prefix {} is too long",
                bech32_prefix_str
            )))
        } else {
            let mut bech32_prefix = [0; BECH32_PREFIX_MAX_LEN];
            let bech32_prefix_vec = Vec::from(bech32_prefix_str);
            for i in 0..bech32_prefix_len {
                bech32_prefix[i] = bech32_prefix_vec[i];
            }
            Ok(RpcMockApi {
                canonical_length,
                bech32_prefix,
                bech32_prefix_len,
            })
        }
    }
}

impl BackendApi for RpcMockApi {
    fn canonical_address(&self, human: &str) -> BackendResult<Vec<u8>> {
        let bech32_prefix = unsafe {
            String::from_utf8_unchecked(self.bech32_prefix[0..self.bech32_prefix_len].to_vec())
        };
        if !human.starts_with(&bech32_prefix) {
            return (
                Err(BackendError::user_err(format!(
                    "Invalid input: human address does not begin with bech32 prefix: {}",
                    human
                ))),
                GasInfo::free(),
            );
        }
        let (hrp, base32_vec, _) = match bech32::decode(human) {
            Ok(a) => a,
            Err(e) => {
                return (
                    Err(BackendError::UserErr {
                        msg: format!(
                            "Invalid input: human address is not bech32 decodable: {}",
                            human
                        ),
                    }),
                    GasInfo::free(),
                );
            }
        };
        let out = Vec::<u8>::from_base32(&base32_vec).unwrap();
        (Ok(out), GasInfo::free())
    }

    fn human_address(&self, canonical: &[u8]) -> BackendResult<String> {
        let bech32_prefix = unsafe {
            String::from_utf8_unchecked(self.bech32_prefix[0..self.bech32_prefix_len].to_vec())
        };
        if canonical.len() != self.canonical_length {
            return (
                Err(BackendError::user_err(
                    "Invalid input: canonical address length not correct",
                )),
                GasInfo::free(),
            );
        }

        // decode UTF-8 bytes into string
        if let Ok(human) = bech32::encode(
            bech32_prefix.as_str(),
            canonical.to_base32(),
            Variant::Bech32,
        ) {
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
