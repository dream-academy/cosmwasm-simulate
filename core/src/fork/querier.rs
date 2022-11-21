use crate::fork::AllStates;
use crate::{ContractState, DebugLog, Error, RpcContractInstance, RpcMockApi, RpcMockStorage};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Addr, Binary, ContractInfo, ContractResult, Env,
    QueryRequest, SystemResult, WasmQuery,
};
use cosmwasm_vm::{Backend, BackendError, BackendResult, GasInfo, InstanceOptions, Querier};
use serde::{Deserialize, Serialize};

use std::sync::{Arc, Mutex, RwLock};

use super::model::maybe_unzip;

#[derive(Clone)]
pub struct RpcMockQuerier {
    states: Arc<RwLock<AllStates>>,
    debug_log: Arc<Mutex<DebugLog>>,
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

impl RpcMockQuerier {
    fn fetch_contract_state(&self, contract_addr: &Addr) -> Result<(), Error> {
        if self
            .states
            .read()
            .unwrap()
            .contract_state_get(contract_addr)
            .is_some()
        {
            return Ok(());
        }
        let contract_info = self
            .states
            .write()
            .unwrap()
            .client
            .query_wasm_contract_info(contract_addr.as_str())?;
        let wasm_code = maybe_unzip(
            self.states
                .write()
                .unwrap()
                .client
                .query_wasm_contract_code(contract_info.code_id)?,
        )?;
        let contract_state = ContractState {
            code: wasm_code,
            storage: Arc::new(RwLock::new(
                self.states
                    .write()
                    .unwrap()
                    .client
                    .query_wasm_contract_state_all(contract_addr.as_str())?,
            )),
        };
        self.states
            .write()
            .unwrap()
            .contract_state_insert(contract_addr.clone(), contract_state);
        Ok(())
    }

    fn env(&self, contract_addr: &Addr) -> Result<Env, Error> {
        let states = self.states.read().unwrap();
        let block_number = states.block_number;
        let block_timestamp = states.block_timestamp.clone();
        let chain_id = states.chain_id.to_string();
        Ok(Env {
            block: cosmwasm_std::BlockInfo {
                height: block_number,
                time: block_timestamp,
                chain_id,
            },
            // assumption: all blocks have only 1 transaction
            transaction: Some(cosmwasm_std::TransactionInfo { index: 0 }),
            // I don't really know what this is for, so for now, set it to the target contract address
            contract: ContractInfo {
                address: contract_addr.clone(),
            },
        })
    }

    fn mock_storage(&self, contract_state: &ContractState) -> Result<RpcMockStorage, Error> {
        let storage = RpcMockStorage::new(&contract_state.storage);
        Ok(storage)
    }
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
                match self.states.write().unwrap().bank_query(&bank_query) {
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
                } else {
                    if let Err(e) = self.fetch_contract_state(&contract_addr) {
                        return (
                            Err(BackendError::Unknown { msg: e.to_string() }),
                            GasInfo::free(),
                        );
                    }
                    let env = match self.env(&contract_addr) {
                        Ok(e) => e,
                        Err(e) => {
                            return (
                                Err(BackendError::Unknown { msg: e.to_string() }),
                                GasInfo::free(),
                            );
                        }
                    };
                    let contract_state = self
                        .states
                        .read()
                        .unwrap()
                        .contract_state_get(&contract_addr)
                        .unwrap()
                        .clone();
                    let states = self.states.read().unwrap();
                    let canonical_address_length = states.canonical_address_length;
                    let bech32_prefix = states.bech32_prefix.to_string();
                    let storage = match self.mock_storage(&contract_state) {
                        Ok(s) => s,
                        Err(e) => {
                            return (
                                Err(BackendError::Unknown { msg: e.to_string() }),
                                GasInfo::free(),
                            );
                        }
                    };
                    let api =
                        match RpcMockApi::new(canonical_address_length, bech32_prefix.as_str()) {
                            Ok(a) => a,
                            Err(e) => {
                                return (
                                    Err(BackendError::Unknown { msg: e.to_string() }),
                                    GasInfo::free(),
                                );
                            }
                        };
                    let deps = Backend {
                        storage,
                        api,
                        querier: RpcMockQuerier::new(&self.states, &self.debug_log),
                    };
                    let options = InstanceOptions {
                        gas_limit: u64::MAX,
                        print_debug: false,
                    };
                    let wasm_instance = match cosmwasm_vm::Instance::from_code(
                        contract_state.code.as_slice(),
                        deps,
                        options,
                        None,
                    ) {
                        Err(e) => {
                            return (
                                Err(BackendError::Unknown { msg: e.to_string() }),
                                GasInfo::free(),
                            );
                        }
                        Ok(i) => i,
                    };
                    let mut instance = RpcContractInstance::new(&contract_addr, wasm_instance);
                    match instance.query(&env, &wasm_query) {
                        Ok(response) => (
                            Ok(SystemResult::Ok(ContractResult::Ok(response))),
                            GasInfo::free(),
                        ),
                        Err(e) => (
                            Err(BackendError::Unknown { msg: e.to_string() }),
                            GasInfo::free(),
                        ),
                    }
                }
            }
            _ => unimplemented!(),
        }
    }
}

impl RpcMockQuerier {
    pub fn new(states: &Arc<RwLock<AllStates>>, debug_log: &Arc<Mutex<DebugLog>>) -> Self {
        Self {
            states: states.clone(),
            debug_log: debug_log.clone(),
        }
    }
}
