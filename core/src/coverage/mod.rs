use std::collections::HashMap;

use crate::{Error, Model, RpcContractInstance};
use cosmwasm_vm::call_raw;

static COVERAGE_MAX_LEN: usize = 0x200000;

#[derive(Clone)]
pub struct CoverageInfo {
    enabled: bool,
    coverage_data: HashMap<String, Vec<Vec<u8>>>,
}

impl CoverageInfo {
    pub fn new() -> Self {
        Self {
            enabled: false,
            coverage_data: HashMap::new(),
        }
    }

    pub fn get_coverage(&self) -> HashMap<String, Vec<Vec<u8>>> {
        self.coverage_data.clone()
    }

    fn add_coverage(&mut self, address: String, cov_data: Vec<u8>) {
        self.coverage_data
            .entry(address)
            .or_insert_with(Vec::new)
            .push(cov_data);
    }
}

impl Model {
    pub fn enable_code_coverage(&mut self) {
        self.coverage_info.enabled = true;
    }
    pub fn disable_code_coverage(&mut self) {
        self.coverage_info.enabled = true;
    }
    pub fn handle_coverage(&mut self, instance: &mut RpcContractInstance) -> Result<(), Error> {
        if self.coverage_info.enabled {
            let cov = instance.dump_coverage()?;
            self.coverage_info
                .add_coverage(instance.address().to_string(), cov);
        }
        Ok(())
    }
    pub fn get_coverage(&self) -> HashMap<String, Vec<Vec<u8>>> {
        self.coverage_info.get_coverage()
    }
}

impl RpcContractInstance {
    pub fn dump_coverage(&mut self) -> Result<Vec<u8>, Error> {
        let result = match call_raw(&mut self.instance, "dump_coverage", &[], COVERAGE_MAX_LEN) {
            Ok(r) => r,
            Err(_e) => {
                // for now, just ignore warnings
                Vec::new()
            }
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::to_binary;
    use test_contract_cov::msg::InstantiateMsg;

    use crate::Model;

    const MALAGA_RPC_URL: &str = "https://rpc.malaga-420.cosmwasm.com:443";
    const MALAGA_BLOCK_NUMBER: u64 = 2326474;

    #[test]
    fn test_collect_coverage_single() {
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER), "wasm").unwrap();
        model.enable_code_coverage();
        let wasm_code = include_bytes!(concat!(
            env!("OUT_DIR"),
            "/wasm32-unknown-unknown/release/test_contract_cov.wasm"
        ));
        model.add_custom_code(1337, wasm_code).unwrap();
        let msg = to_binary(&InstantiateMsg {}).unwrap();
        let _ = model.instantiate(1337, msg.as_slice(), &[]).unwrap();
        assert!(model.get_coverage().len() > 0);
    }
}
