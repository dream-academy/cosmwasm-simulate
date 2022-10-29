use cosmwasm_std::{Addr, ContractInfo, ContractResult, Env, MessageInfo, Response, Timestamp, Coin};
use cosmwasm_vm::{call_execute, call_instantiate, Instance, BackendApi};

use crate::contract_vm::rpc_mock::{querier::RpcMockQuerier, RpcMockApi, RpcMockStorage};
use crate::contract_vm::Error;

type RpcInstance = Instance<RpcMockApi, RpcMockStorage, RpcMockQuerier>;

pub struct RpcContractInstance {
    instance: RpcInstance,
}

impl<'a> RpcContractInstance {
    pub fn make_instance(instance: RpcInstance) -> Self {
        Self { instance: instance }
    }

    pub fn execute(&mut self, env: &Env, msg: &[u8], sender: &Addr, funds: &[Coin]) -> Result<Response, Error> {
        let info = MessageInfo {
            sender: sender.clone(),
            funds: funds.to_vec(),
        };
        let handle_result: ContractResult<Response> =
            call_execute(&mut self.instance, env, &info, msg).map_err(|e| Error::vm_error(e))?;
        let response = match handle_result {
            ContractResult::Ok(r) => r,
            ContractResult::Err(e) => {
                return Err(Error::vm_error(e));
            }
        };
        Ok(response)
    }
}
