use crate::{DebugLog, Error, Model, RpcContractInstance};
use cosmwasm_vm::call_raw;
use serde::{Deserialize, Serialize};

static COVERAGE_MAX_LEN: usize = 0x200000;

#[derive(Serialize, Deserialize)]
pub enum QueryMsg {
    DumpCoverage {},
}

impl DebugLog {
    pub fn get_code_coverage_for_address(&self, address: &str) -> Vec<Vec<u8>> {
        if let Some(cov) = self.code_coverage.get(address) {
            cov.clone()
        } else {
            Vec::new()
        }
    }
}

impl Model {
    pub fn enable_code_coverage(&mut self) {
        self.code_coverage_enabled = true;
    }
    pub fn disable_code_coverage(&mut self) {
        self.code_coverage_enabled = false;
    }
    pub fn handle_coverage(&mut self, instance: &mut RpcContractInstance) -> Result<(), Error> {
        if self.code_coverage_enabled {
            let cov = instance.dump_coverage()?;
            self.debug_log
                .lock()
                .unwrap()
                .code_coverage
                .entry(instance.address().to_string())
                .or_insert_with(Vec::new)
                .push(cov);
        }
        Ok(())
    }
}

impl RpcContractInstance {
    pub fn dump_coverage(&mut self) -> Result<Vec<u8>, Error> {
        let result = call_raw(&mut self.instance, "dump_coverage", &[], COVERAGE_MAX_LEN)
            .map_err(Error::vm_error)?;
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
        let debug_log = model.instantiate(1337, msg.as_slice(), &[]).unwrap();
        assert!(debug_log.code_coverage.len() > 0);
    }
}
