use cosmwasm_std::{
    to_binary, Addr, Binary, Coin, ContractInfo, ContractResult, Env, MessageInfo, Reply, Response,
    WasmQuery,
};
use cosmwasm_vm::{
    call_execute, call_instantiate, call_query, call_reply, Instance, Storage, VmError,
};

use crate::fork::{querier::RpcMockQuerier, RpcBackend, RpcMockApi, RpcMockStorage};
use crate::Error;

pub type RpcInstance = Instance<RpcMockApi, RpcMockStorage, RpcMockQuerier>;

pub struct RpcContractInstance {
    contract_info: ContractInfo,
    pub instance: RpcInstance,
}

impl RpcContractInstance {
    pub fn new(address: &Addr, instance: RpcInstance) -> Self {
        let contract_info = ContractInfo {
            address: address.clone(),
        };
        Self {
            contract_info,
            instance,
        }
    }

    pub fn address(&self) -> Addr {
        self.contract_info.address.clone()
    }

    pub fn instantiate(
        &mut self,
        env: &Env,
        msg: &[u8],
        sender: &Addr,
        funds: &[Coin],
    ) -> Result<ContractResult<Response>, Error> {
        let info = MessageInfo {
            sender: sender.clone(),
            funds: funds.to_vec(),
        };
        call_instantiate(&mut self.instance, env, &info, msg).map_err(Error::vm_error)
    }

    pub fn execute(
        &mut self,
        env: &Env,
        msg: &[u8],
        sender: &Addr,
        funds: &[Coin],
    ) -> Result<ContractResult<Response>, Error> {
        let info = MessageInfo {
            sender: sender.clone(),
            funds: funds.to_vec(),
        };
        call_execute(&mut self.instance, env, &info, msg).map_err(Error::vm_error)
    }

    pub fn reply(&mut self, env: &Env, msg: &Reply) -> Result<ContractResult<Response>, Error> {
        call_reply(&mut self.instance, env, msg).map_err(Error::vm_error)
    }

    pub fn query(&mut self, env: &Env, wasm_query: &WasmQuery) -> Result<Binary, Error> {
        match wasm_query {
            WasmQuery::ContractInfo { contract_addr: _ } => {
                Ok(to_binary(&self.contract_info).unwrap())
            }
            WasmQuery::Raw {
                contract_addr: _,
                key,
            } => {
                if let Some(value) = self
                    .instance
                    .with_storage(|s| {
                        let (res, _) = s.get(key.as_slice());
                        match res {
                            Ok(res) => Ok(res),
                            Err(e) => Err(VmError::BackendErr { source: e }),
                        }
                    })
                    .map_err(Error::vm_error)?
                {
                    Ok(Binary::from(value.as_slice()))
                } else {
                    Ok(Binary::from(Vec::<u8>::new().as_slice()))
                }
            }
            WasmQuery::Smart {
                contract_addr: _,
                msg,
            } => {
                match call_query(&mut self.instance, env, msg.as_slice())
                    .map_err(Error::vm_error)?
                {
                    ContractResult::Ok(r) => Ok(r),
                    ContractResult::Err(e) => Err(Error::vm_error(&e)),
                }
            }
            _ => unimplemented!(),
        }
    }

    pub fn recycle(self) -> RpcBackend {
        // this cannot panic, because all instances have storage and api
        self.instance.recycle().unwrap()
    }

    pub fn storage_write(&mut self, key: &[u8], value: &[u8]) -> Result<(), Error> {
        self.instance
            .with_storage(|s| {
                let (b, _) = s.set(key, value);
                b.map_err(|e| VmError::BackendErr { source: e })
            })
            .map_err(Error::vm_error)?;
        Ok(())
    }
}
