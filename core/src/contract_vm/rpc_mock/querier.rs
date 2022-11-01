use crate::{
    contract_vm::rpc_mock::{Bank, CwRpcClient, RpcContractInstance},
    DebugLog,
};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Addr, Binary, BlockInfo, ContractInfo, ContractResult, Env,
    QueryRequest, SystemResult, Timestamp, WasmQuery,
};
use cosmwasm_vm::{BackendError, BackendResult, GasInfo, Querier};
use serde::{Deserialize, Serialize};

use dashmap::DashMap;
use std::sync::{Arc, Mutex};

type Instances = DashMap<Addr, RpcContractInstance>;

#[derive(Clone)]
pub struct RpcMockQuerier {
    client: Arc<Mutex<CwRpcClient>>,
    bank: Arc<Mutex<Bank>>,
    debug_log: Arc<Mutex<DebugLog>>,
    instances: Arc<Instances>,
}

const PRINTER_ADDR: &str = "supergodprinter";

#[derive(Serialize, Deserialize)]
struct PrintRequest {
    msg: String,
}

#[derive(Serialize, Deserialize)]
struct PrintResponse {
    ack: bool,
}

impl Querier for RpcMockQuerier {
    fn query_raw(
        &self,
        request: &[u8],
        _gas_limit: u64,
    ) -> BackendResult<SystemResult<ContractResult<Binary>>> {
        let request: QueryRequest<()> = match from_slice(request) {
            Ok(v) => v,
            Err(e) => {
                return (
                    Err(BackendError::Unknown { msg: e.to_string() }),
                    GasInfo::free(),
                )
            }
        };
        match request {
            QueryRequest::Bank(bank_query) => {
                let mut bank = self.bank.lock().unwrap();
                match bank.query(&bank_query) {
                    Ok(resp) => {
                        (
                            // wait, is this correct?
                            Ok(SystemResult::Ok(ContractResult::Ok(resp))),
                            GasInfo::free(),
                        )
                    }
                    Err(e) => (
                        Err(BackendError::Unknown { msg: e.to_string() }),
                        GasInfo::free(),
                    ),
                }
            }
            QueryRequest::Wasm(wasm_query) => {
                let contract_addr = Addr::unchecked(match &wasm_query {
                    WasmQuery::ContractInfo { contract_addr } => contract_addr,
                    WasmQuery::Raw { contract_addr, .. } => contract_addr,
                    WasmQuery::Smart { contract_addr, .. } => contract_addr,
                    _ => unimplemented!(),
                });
                if contract_addr.as_str() == PRINTER_ADDR {
                    match wasm_query {
                        WasmQuery::Smart {
                            contract_addr: _,
                            msg,
                        } => {
                            let msg: PrintRequest = from_binary(&msg).unwrap();
                            self.debug_log.lock().unwrap().append_stdout(&msg.msg);
                            let resp = to_binary(&PrintResponse { ack: true }).unwrap();
                            (
                                Ok(SystemResult::Ok(ContractResult::Ok(resp))),
                                GasInfo::free(),
                            )
                        }
                        _ => {
                            panic!("invalid query to printer");
                        }
                    }
                } else if let Some(mut instance) = self.instances.get_mut(&contract_addr) {
                    let env = Env {
                        block: BlockInfo {
                            // TODO: fix
                            height: 0,
                            time: Timestamp::from_nanos(0),
                            chain_id: "chaind".to_string(),
                        },
                        transaction: None,
                        contract: ContractInfo {
                            address: Addr::unchecked(contract_addr),
                        },
                    };
                    match instance.query(&env, &wasm_query) {
                        Ok(b) => (Ok(SystemResult::Ok(ContractResult::Ok(b))), GasInfo::free()),
                        Err(e) => (
                            Err(BackendError::Unknown { msg: e.to_string() }),
                            GasInfo::free(),
                        ),
                    }
                } else {
                    // instance is not created by model
                    // since we do not have access to model, we can't create a new instance from here
                    // but, it doesn't affect the integrity of the model, because whenever cheat codes are invoked,
                    // it creates an instance and thus it must exist in the instances hashmap
                    let mut client = self.client.lock().unwrap();
                    match &wasm_query {
                        WasmQuery::ContractInfo { contract_addr: _ } => {
                            unimplemented!()
                        }
                        WasmQuery::Raw { contract_addr, key } => {
                            let states = match client.query_wasm_contract_all(contract_addr) {
                                Ok(s) => s,
                                Err(e) => {
                                    return (
                                        Err(BackendError::Unknown { msg: e.to_string() }),
                                        GasInfo::free(),
                                    );
                                }
                            };
                            let key = key.to_vec();
                            if let Some(value) = states.get(&key) {
                                (
                                    Ok(SystemResult::Ok(ContractResult::Ok(Binary::from(
                                        value.as_slice(),
                                    )))),
                                    GasInfo::free(),
                                )
                            } else {
                                (
                                    Ok(SystemResult::Ok(ContractResult::Ok(Binary::from(
                                        vec![].as_slice(),
                                    )))),
                                    GasInfo::free(),
                                )
                            }
                        }
                        WasmQuery::Smart { contract_addr, msg } => {
                            let response = match client
                                .query_wasm_contract_smart(contract_addr, msg.as_slice())
                            {
                                Ok(resp) => resp,
                                Err(e) => {
                                    return (
                                        Err(BackendError::Unknown { msg: e.to_string() }),
                                        GasInfo::free(),
                                    );
                                }
                            };
                            (
                                Ok(SystemResult::Ok(ContractResult::Ok(Binary::from(
                                    response.as_slice(),
                                )))),
                                GasInfo::free(),
                            )
                        }
                        _ => unimplemented!(),
                    }
                }
            }
            _ => unimplemented!(),
        }
    }
}

impl RpcMockQuerier {
    pub fn new(
        client: &Arc<Mutex<CwRpcClient>>,
        bank: &Arc<Mutex<Bank>>,
        debug_log: &Arc<Mutex<DebugLog>>,
        instances: &Arc<Instances>,
    ) -> Self {
        Self {
            client: Arc::clone(client),
            bank: Arc::clone(bank),
            debug_log: Arc::clone(debug_log),
            instances: Arc::clone(instances),
        }
    }
}
