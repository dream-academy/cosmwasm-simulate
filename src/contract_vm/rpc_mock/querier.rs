use crate::contract_vm::rpc_mock::Coin;
use cosmwasm_std::{Binary, ContractResult, SystemResult};
use cosmwasm_vm::{BackendResult, Querier};
pub struct RpcMockQuerier {}

impl Querier for RpcMockQuerier {
    fn query_raw(
        &self,
        request: &[u8],
        gas_limit: u64,
    ) -> BackendResult<SystemResult<ContractResult<Binary>>> {
        unimplemented!()
    }
}

impl RpcMockQuerier {
    pub fn new(balances: &[(&str, &[Coin])]) -> Self {
        unimplemented!()
    }
}
