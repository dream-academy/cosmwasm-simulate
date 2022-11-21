use crate::Error;
use cosmwasm_std::Timestamp;
use std::collections::BTreeMap;

/// Full contract_info is much more verbose, and contains fields such as admin, creator, label, etc
/// However, those fields are not used for simulations, and thus neglected for now
pub struct ContractInfo {
    pub code_id: u64,
}
pub trait CwClientBackend: CwClientBackendClone + Send + Sync {
    fn block_number(&self) -> u64;
    fn chain_id(&mut self) -> Result<String, Error>;
    fn timestamp(&mut self) -> Result<Timestamp, Error>;
    fn block_height(&mut self) -> Result<u64, Error>;
    fn query_bank_all_balances(&mut self, address: &str) -> Result<Vec<(String, u128)>, Error>;
    fn query_wasm_contract_smart(
        &mut self,
        address: &str,
        query_data: &[u8],
    ) -> Result<Vec<u8>, Error>;
    fn query_wasm_contract_state_all(
        &mut self,
        address: &str,
    ) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, Error>;
    fn query_wasm_contract_info(&mut self, address: &str) -> Result<ContractInfo, Error>;
    fn query_wasm_contract_code(&mut self, code_id: u64) -> Result<Vec<u8>, Error>;
}

pub trait CwClientBackendClone {
    fn clone_box(&self) -> Box<dyn CwClientBackend>;
}

impl<T> CwClientBackendClone for T
where
    T: 'static + CwClientBackend + Clone,
{
    fn clone_box(&self) -> Box<dyn CwClientBackend> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn CwClientBackend> {
    fn clone(&self) -> Box<dyn CwClientBackend> {
        self.clone_box()
    }
}
