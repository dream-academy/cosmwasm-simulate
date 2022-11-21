use bech32::{self, FromBase32, ToBase32, Variant};
use cosmwasm_vm::{BackendApi, BackendError, BackendResult, GasInfo};

use crate::Error;

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
            bech32_prefix[..bech32_prefix_len]
                .copy_from_slice(&bech32_prefix_vec[..bech32_prefix_len]);
            Ok(RpcMockApi {
                canonical_length,
                bech32_prefix,
                bech32_prefix_len,
            })
        }
    }
}

pub fn human_to_canonical(human: &str, bech32_prefix: &str) -> Result<Vec<u8>, String> {
    if !human.starts_with(bech32_prefix) {
        return Err(format!(
            "Invalid input: human address does not begin with bech32 prefix: {}",
            human
        ));
    }
    let (hrp, base32_vec, _) = match bech32::decode(human) {
        Ok(a) => a,
        Err(e) => {
            return Err(format!(
                "Invalid input: human address is not bech32 decodable: {}",
                e
            ));
        }
    };
    if hrp != bech32_prefix {
        Err(format!(
            "Invalid input: human address has invalid bech32 prefix: {}",
            hrp
        ))
    } else {
        // canonical addresses can either be 20 bytes or 32 bytes
        let out = Vec::<u8>::from_base32(&base32_vec).unwrap();
        Ok(out)
    }
}

pub fn canonical_to_human(
    canonical: &[u8],
    bech32_prefix: &str,
    canon_length: usize,
) -> Result<String, String> {
    // canonical addresses can either be 20 bytes or 32 bytes
    if canonical.len() > canon_length {
        return Err("Invalid input: canonical address length not correct".to_string());
    }
    // decode UTF-8 bytes into string
    if let Ok(human) = bech32::encode(bech32_prefix, canonical.to_base32(), Variant::Bech32) {
        Ok(human)
    } else {
        Err("Invalid input: canonical address not encodable".to_string())
    }
}

impl BackendApi for RpcMockApi {
    fn canonical_address(&self, human: &str) -> BackendResult<Vec<u8>> {
        let bech32_prefix = unsafe {
            String::from_utf8_unchecked(self.bech32_prefix[0..self.bech32_prefix_len].to_vec())
        };
        match human_to_canonical(human, bech32_prefix.as_str()) {
            Ok(c) => (Ok(c), GasInfo::free()),
            Err(e) => (Err(BackendError::user_err(e)), GasInfo::free()),
        }
    }

    fn human_address(&self, canonical: &[u8]) -> BackendResult<String> {
        let bech32_prefix = unsafe {
            String::from_utf8_unchecked(self.bech32_prefix[0..self.bech32_prefix_len].to_vec())
        };
        match canonical_to_human(canonical, bech32_prefix.as_str(), self.canonical_length) {
            Ok(h) => (Ok(h), GasInfo::free()),
            Err(e) => (Err(BackendError::user_err(e)), GasInfo::free()),
        }
    }
}
