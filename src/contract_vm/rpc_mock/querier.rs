use crate::contract_vm::rpc_mock::rpc::CwRpcClient;
use cosmwasm_std::{Binary, ContractResult, SystemResult};
use cosmwasm_vm::{BackendResult, Querier};

pub struct RpcMockQuerier {
    client: CwRpcClient,
}

impl<'a> Querier for RpcMockQuerier {
    fn query_raw(
        &self,
        request: &[u8],
        gas_limit: u64,
    ) -> BackendResult<SystemResult<ContractResult<Binary>>> {
        unimplemented!()
    }
}

impl<'a> RpcMockQuerier {
    pub fn new(client: &CwRpcClient) -> Self {
        Self {
            client: client.clone(),
        }
    }
}
