use crate::coverage::CoverageInfo;
use crate::fork::api::canonical_to_human;
use crate::{
    rpc_items, AllStates, ContractState, ContractStorage, CwClientBackend, CwRpcClient, DebugLog,
    Error, RpcContractInstance, RpcInstance, RpcMockApi, RpcMockQuerier, RpcMockStorage,
};

use cosmwasm_std::{
    from_binary, Addr, BankMsg, BankQuery, Binary, Coin, ContractInfo, ContractResult, CosmosMsg,
    Env, Event, Reply, ReplyOn, Response, SubMsgResponse, SubMsgResult, Timestamp, Uint128,
    WasmMsg, WasmQuery,
};
use cosmwasm_vm::internals::instance_from_module;
use cosmwasm_vm::{Backend, InstanceOptions};
use prost::Message;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::mem;
use std::sync::{Arc, Mutex, RwLock};
use wasmer::Module;

use super::lcd::CwLcdClient;

pub type RpcBackend = Backend<RpcMockApi, RpcMockStorage, RpcMockQuerier>;

pub struct Model {
    states: Arc<RwLock<AllStates>>,
    // similar to tx.origin of solidity
    sender: String,
    // used to generate addresses in instantiate
    code_id_counters: HashMap<u64, u64>,
    // for debugging
    pub debug_log: Arc<Mutex<DebugLog>>,
    // for userprovided code
    custom_codes: HashMap<u64, Vec<u8>>,
    // for code coverage
    pub coverage_info: CoverageInfo,
    // for saving webassembly compilation time
    pub wasm_cache: HashMap<Vec<u8>, Module>,
}

const WASM_MAGIC: [u8; 4] = [0, 97, 115, 109];
const GZIP_MAGIC: [u8; 4] = [0, 0, 0, 0];
const BASE_EOA: &str = "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj";

pub fn maybe_unzip(input: Vec<u8>) -> Result<Vec<u8>, Error> {
    let magic = &input[0..4];
    if magic == WASM_MAGIC {
        Ok(input)
    } else if magic == GZIP_MAGIC {
        unimplemented!();
    } else {
        eprintln!("unidentifiable magic: {:?}", magic);
        unimplemented!();
    }
}

impl Clone for Model {
    fn clone(&self) -> Self {
        Model {
            states: Arc::new(RwLock::new(self.states.read().unwrap().clone())),
            sender: self.sender.clone(),
            code_id_counters: self.code_id_counters.clone(),
            debug_log: Arc::new(Mutex::new(self.debug_log.lock().unwrap().clone())),
            custom_codes: self.custom_codes.clone(),
            coverage_info: self.coverage_info.clone(),
            wasm_cache: self.wasm_cache.clone(),
        }
    }
}

impl Model {
    pub fn new_lcd(url: &str, bech32_prefix: &str) -> Result<Self, Error> {
        let client: Box<dyn CwClientBackend> = Box::new(CwLcdClient::new(url)?);
        Ok(Model {
            states: Arc::new(RwLock::new(AllStates::new(client, 32, bech32_prefix)?)),
            sender: BASE_EOA.to_string(),
            code_id_counters: HashMap::new(),
            debug_log: Arc::new(Mutex::new(DebugLog::new())),
            custom_codes: HashMap::new(),
            coverage_info: CoverageInfo::new(),
            wasm_cache: HashMap::new(),
        })
    }

    pub fn new(url: &str, block_number: Option<u64>, bech32_prefix: &str) -> Result<Self, Error> {
        // for now, let's not use LCD and default to RPC
        let client: Box<dyn CwClientBackend> = Box::new(CwRpcClient::new(url, block_number)?);
        Ok(Model {
            states: Arc::new(RwLock::new(AllStates::new(client, 32, bech32_prefix)?)),
            sender: BASE_EOA.to_string(),
            code_id_counters: HashMap::new(),
            debug_log: Arc::new(Mutex::new(DebugLog::new())),
            custom_codes: HashMap::new(),
            coverage_info: CoverageInfo::new(),
            wasm_cache: HashMap::new(),
        })
    }

    pub fn block_number(&self) -> u64 {
        self.states.read().unwrap().client.block_number()
    }

    /// Does nothing if the state already exists
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

    fn generate_address(&mut self, code_id: u64) -> Result<Addr, Error> {
        let code_id_counter = self.code_id_counters.entry(code_id).or_insert(0);
        let seed = format!("seeeed_{}_{}", code_id, *code_id_counter);
        // TODO: counter must not be incremented if instantiation fails
        *code_id_counter += 1;
        let mut hasher = Sha256::new();
        hasher.update(seed);
        let bytes = hasher.finalize();
        let addr = canonical_to_human(
            bytes.as_slice(),
            &self.states.read().unwrap().bech32_prefix,
            self.states.read().unwrap().canonical_address_length,
        )
        .map_err(|e| Error::format_error(&e))?;
        Ok(Addr::unchecked(addr))
    }

    fn revert(&mut self, prev_state: Model) -> Model {
        // don't revert coverage state
        let cur_state: Model = mem::replace(self, prev_state);
        self.coverage_info = cur_state.coverage_info.clone();
        cur_state
    }

    fn create_instance(&self, contract_addr: &Addr) -> Result<RpcContractInstance, Error> {
        self.fetch_contract_state(contract_addr)?;
        let states = self.states.read().unwrap();
        let contract_state = states.contract_state_get(contract_addr).unwrap();
        let deps = self.new_mock(&contract_state.storage)?;
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
                return Err(Error::vm_error(e));
            }
            Ok(i) => i,
        };
        Ok(RpcContractInstance::new(contract_addr, wasm_instance))
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_submessage_instantiate(
        &mut self,
        origin: &Addr,
        admin: &Option<String>,
        code_id: u64,
        msg: &Binary,
        funds: &[Coin],
        sub_msg_id: u64,
        reply_on: &ReplyOn,
    ) -> Result<ContractResult<Response>, Error> {
        let (response, new_addr) = match admin {
            Some(allowed) => {
                if allowed != origin {
                    (
                        ContractResult::Err("cannot instantiate contract".to_string()),
                        None,
                    )
                } else {
                    let (res, new_addr) = self.instantiate_inner(code_id, origin, msg, funds)?;
                    (res, new_addr)
                }
            }
            None => {
                let (res, new_addr) = self.instantiate_inner(code_id, origin, msg, funds)?;
                (res, new_addr)
            }
        };
        let do_reply = match reply_on {
            ReplyOn::Always => true,
            ReplyOn::Success => response.is_ok(),
            ReplyOn::Error => response.is_err(),
            ReplyOn::Never => false,
        };
        if do_reply {
            let data = rpc_items::cosmwasm::wasm::v1::MsgInstantiateContractResponse {
                address: if let Some(a) = new_addr {
                    a.to_string()
                } else {
                    "".to_string()
                },
                data: Vec::new(),
            };
            let env = self.env(origin)?;
            let reply = Reply {
                id: sub_msg_id,
                result: match response {
                    ContractResult::Ok(r) => SubMsgResult::Ok(SubMsgResponse {
                        events: r.events,
                        data: Some(Binary::from(Message::encode_to_vec(&data))),
                    }),
                    ContractResult::Err(e) => SubMsgResult::Err(e),
                },
            };

            let mut instance = self.create_instance(origin)?;

            // open new call context
            let call_id = self.debug_log.lock().unwrap().begin_reply(origin, msg);

            let maybe_response = instance.reply(&env, &reply)?;
            self.handle_coverage(&mut instance)?;

            if let ContractResult::Err(e) = &maybe_response {
                // propagate error. instance.reply need not error handling
                // no need to re-insert the instance
                self.debug_log.lock().unwrap().begin_error(e);
                Ok(maybe_response)
            } else {
                let response = maybe_response.unwrap();
                self.debug_log.lock().unwrap().append_log(&response);
                let response = self.handle_response(origin, &response)?;
                // close call context
                self.debug_log.lock().unwrap().end_reply(call_id);
                Ok(response)
            }
        }
        // if reply is not called, but the current result is an error, propagate the error
        else if let ContractResult::Err(e) = &response {
            self.debug_log.lock().unwrap().begin_error(e);
            Ok(ContractResult::Err(response.unwrap_err()))
        }
        // otherwise, recursively handle the submessages
        else {
            self.handle_response(origin, &response.unwrap())
        }
    }

    fn handle_submessage_execute(
        &mut self,
        origin: &Addr,
        target_addr: &Addr,
        msg: &Binary,
        funds: &[Coin],
        sub_msg_id: u64,
        reply_on: &ReplyOn,
    ) -> Result<ContractResult<Response>, Error> {
        let response = self.execute_inner(target_addr, origin, msg.as_slice(), funds)?;
        let do_reply = match reply_on {
            ReplyOn::Always => true,
            ReplyOn::Success => response.is_ok(),
            ReplyOn::Error => response.is_err(),
            ReplyOn::Never => false,
        };
        if do_reply {
            let data =
                rpc_items::cosmwasm::wasm::v1::MsgExecuteContractResponse { data: Vec::new() };
            let env = self.env(origin)?;
            let reply = Reply {
                id: sub_msg_id,
                result: match response {
                    ContractResult::Ok(r) => SubMsgResult::Ok(SubMsgResponse {
                        events: r.events,
                        data: Some(Binary::from(Message::encode_to_vec(&data))),
                    }),
                    ContractResult::Err(e) => SubMsgResult::Err(e),
                },
            };

            let mut instance = self.create_instance(origin)?;

            // open new call context
            let call_id = self.debug_log.lock().unwrap().begin_reply(origin, msg);

            let maybe_response = instance.reply(&env, &reply)?;
            self.handle_coverage(&mut instance)?;

            if let ContractResult::Err(e) = &maybe_response {
                // propagate error. instance.reply need not error handling
                // no need to re-insert the instance
                self.debug_log.lock().unwrap().begin_error(e);
                Ok(maybe_response)
            } else {
                let response = maybe_response.unwrap();
                self.debug_log.lock().unwrap().append_log(&response);
                let response = self.handle_response(origin, &response)?;
                // close call context
                self.debug_log.lock().unwrap().end_reply(call_id);
                Ok(response)
            }
        }
        // if reply is not called, but the current result is an error, propagate the error
        else if let ContractResult::Err(e) = &response {
            self.debug_log.lock().unwrap().begin_error(e);
            Ok(ContractResult::Err(response.unwrap_err()))
        }
        // otherwise, recursively handle the submessages
        else {
            self.handle_response(origin, &response.unwrap())
        }
    }

    fn handle_response(
        &mut self,
        origin: &Addr,
        response: &Response,
    ) -> Result<ContractResult<Response>, Error> {
        // last_response is the response of the latest execution
        // If there are no submessages, this will be returned. Otherwise, response from the submessages will be returned
        if response.messages.is_empty() {
            return Ok(ContractResult::Ok(response.clone()));
        }
        // this will be overwritten at least once
        let mut last_response = ContractResult::Ok(Response::new());
        // otherwise, execute the submessages
        for sub_msg in response.messages.iter() {
            let response = match &sub_msg.msg {
                CosmosMsg::Wasm(wasm_msg) => match wasm_msg {
                    WasmMsg::Instantiate {
                        admin,
                        code_id,
                        msg,
                        funds,
                        label: _,
                    } => self.handle_submessage_instantiate(
                        origin,
                        admin,
                        *code_id,
                        msg,
                        funds,
                        sub_msg.id,
                        &sub_msg.reply_on,
                    )?,
                    WasmMsg::Execute {
                        contract_addr: target_addr,
                        msg,
                        funds,
                    } => self.handle_submessage_execute(
                        origin,
                        &Addr::unchecked(target_addr),
                        msg,
                        funds,
                        sub_msg.id,
                        &sub_msg.reply_on,
                    )?,
                    _ => unimplemented!(),
                },
                CosmosMsg::Bank(bank_msg) => {
                    // if bank fails, revert the entire transaction
                    self.states
                        .write()
                        .unwrap()
                        .bank_execute(origin, bank_msg)?
                }
                _ => unimplemented!(),
            };
            if response.is_err() {
                return Ok(response);
            } else {
                last_response = response;
            }
        }
        Ok(last_response)
    }

    pub fn add_custom_code(&mut self, code_id: u64, code: &[u8]) -> Result<(), Error> {
        self.custom_codes.insert(code_id, code.to_vec());
        Ok(())
    }

    pub fn create_instance_from_code(
        &mut self,
        code: &[u8],
        deps: RpcBackend,
        options: InstanceOptions,
    ) -> Result<RpcInstance, Error> {
        use cosmwasm_vm::internals::compile;
        let mut hasher = Sha256::new();
        hasher.update(code);
        let code_hash = hasher.finalize().to_vec();
        let module = if let Some(module) = self.wasm_cache.get(&code_hash) {
            module.clone()
        } else {
            let module = compile(code, None, &[]).map_err(Error::vm_error)?;
            self.wasm_cache.insert(code_hash, module.clone());
            module
        };
        match instance_from_module(&module, deps, options.gas_limit, options.print_debug, None) {
            Err(e) => Err(Error::vm_error(e)),
            Ok(i) => Ok(i),
        }
    }

    pub fn instantiate(
        &mut self,
        code_id: u64,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<DebugLog, Error> {
        let sender = self.sender.clone();
        let empty_log = DebugLog::new();
        let state_copy = self.clone();

        let (res, _) = self.instantiate_inner(code_id, &Addr::unchecked(sender), msg, funds)?;
        if res.is_err() {
            let orig_state = self.revert(state_copy);
            let debug_log: DebugLog =
                mem::replace(&mut orig_state.debug_log.lock().unwrap(), empty_log);
            Ok(debug_log)
        } else {
            self.states.write().unwrap().update_block();
            Ok(mem::replace(&mut self.debug_log.lock().unwrap(), empty_log))
        }
    }

    fn instantiate_inner(
        &mut self,
        // this argument should be removed someday
        code_id: u64,
        sender: &Addr,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<(ContractResult<Response>, Option<Addr>), Error> {
        // generate an address
        let contract_addr = self.generate_address(code_id)?;

        // transfer coins
        if funds.len() > 0 {
            let bank_msg = BankMsg::Send {
                to_address: contract_addr.to_string(),
                amount: funds.to_vec(),
            };
            match self
                .states
                .write()
                .unwrap()
                .bank_execute(sender, &bank_msg)?
            {
                ContractResult::Ok(r) => {
                    self.debug_log.lock().unwrap().append_log(&r);
                }
                ContractResult::Err(e) => {
                    self.debug_log.lock().unwrap().set_err_msg(&e);
                    return Ok((ContractResult::Err(e), None));
                }
            };
        }

        // because contract address does not exist on chain, create mock storage from empty set
        let emtpy_storage = Arc::new(RwLock::new(ContractStorage::new()));
        let deps = self.new_mock(&emtpy_storage)?;
        let options = InstanceOptions {
            gas_limit: u64::MAX,
            print_debug: false,
        };
        let wasm_code = if let Some(code) = self.custom_codes.get(&code_id) {
            code.clone()
        } else {
            maybe_unzip(
                self.states
                    .write()
                    .unwrap()
                    .client
                    .query_wasm_contract_code(code_id)?,
            )?
        };
        let wasm_instance = self.create_instance_from_code(wasm_code.as_slice(), deps, options)?;

        // create a temporary contract_state, which will be deleted if instantiation fails
        let contract_state = ContractState {
            code: wasm_code,
            storage: emtpy_storage,
        };
        self.states
            .write()
            .unwrap()
            .contract_state_insert(contract_addr.clone(), contract_state);
        let mut instance = RpcContractInstance::new(&contract_addr, wasm_instance);
        let env = self.env(&contract_addr)?;

        // open new call context
        let call_id = self
            .debug_log
            .lock()
            .unwrap()
            .begin_instantiate(&contract_addr, msg);

        // propagate contract error downwards
        let result = instance.instantiate(&env, msg, sender, funds)?;
        self.handle_coverage(&mut instance)?;
        let response = match result {
            ContractResult::Ok(r) => {
                let instantiate_event = Event::new("instantiate")
                    .add_attribute("code_id", code_id.to_string())
                    .add_attribute("_contract_address", contract_addr.to_string());
                let r = r.add_event(instantiate_event);
                self.debug_log.lock().unwrap().append_log(&r);
                r
            }
            ContractResult::Err(e) => {
                // remove the temporary contract_state created previously
                self.states
                    .write()
                    .unwrap()
                    .contract_state_remove(&contract_addr);
                let mut debug_log = self.debug_log.lock().unwrap();
                debug_log.set_err_msg(&e);
                debug_log.begin_error(&e);
                return Ok((ContractResult::Err(e), None));
            }
        };
        let response = self.handle_response(&contract_addr, &response)?;

        // close calling context
        self.debug_log.lock().unwrap().end_instantiate(call_id);
        Ok((response, Some(contract_addr)))
    }

    pub fn execute(
        &mut self,
        contract_addr: &Addr,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<DebugLog, Error> {
        let empty_log = DebugLog::new();
        let sender = self.sender.clone();
        let state_copy = self.clone();
        if self
            .execute_inner(contract_addr, &Addr::unchecked(sender), msg, funds)?
            .is_err()
        {
            let orig_state = self.revert(state_copy);
            let debug_log: DebugLog =
                mem::replace(&mut orig_state.debug_log.lock().unwrap(), empty_log);
            Ok(debug_log)
        } else {
            self.states.write().unwrap().update_block();
            Ok(mem::replace(&mut self.debug_log.lock().unwrap(), empty_log))
        }
    }

    fn execute_inner(
        &mut self,
        contract_addr: &Addr,
        sender: &Addr,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<ContractResult<Response>, Error> {
        let env = self.env(contract_addr)?;
        let mut instance = self.create_instance(contract_addr)?;

        if funds.len() > 0 {
            // transfer coins
            let bank_msg = BankMsg::Send {
                to_address: contract_addr.to_string(),
                amount: funds.to_vec(),
            };
            match self
                .states
                .write()
                .unwrap()
                .bank_execute(sender, &bank_msg)?
            {
                ContractResult::Ok(r) => {
                    self.debug_log.lock().unwrap().append_log(&r);
                }
                ContractResult::Err(e) => {
                    self.debug_log.lock().unwrap().set_err_msg(&e);
                    return Ok(ContractResult::Err(e));
                }
            };
        }

        // open new call context
        let call_id = self
            .debug_log
            .lock()
            .unwrap()
            .begin_execute(contract_addr, msg);

        // execute contract code
        // propagate contract error downwards
        let result = instance.execute(&env, msg, sender, funds)?;
        self.handle_coverage(&mut instance)?;
        let response = match result {
            ContractResult::Ok(r) => {
                self.debug_log.lock().unwrap().append_log(&r);
                r
            }
            ContractResult::Err(e) => {
                let mut debug_log = self.debug_log.lock().unwrap();
                debug_log.set_err_msg(&e);
                debug_log.begin_error(&e);
                return Ok(ContractResult::Err(e));
            }
        };
        let response = self.handle_response(contract_addr, &response)?;

        // close calling context
        self.debug_log.lock().unwrap().end_execute(call_id);
        Ok(response)
    }

    /// for now, only support WASM queries
    pub fn wasm_query(&mut self, contract_addr: &Addr, msg: &[u8]) -> Result<Binary, Error> {
        let env = self.env(contract_addr)?;
        let mut instance = self.create_instance(contract_addr)?;
        let wasm_query = WasmQuery::Smart {
            contract_addr: contract_addr.to_string(),
            msg: Binary::from(msg),
        };
        // TODO: fix this, propagate contract error down
        let result = instance.query(&env, &wasm_query);
        self.handle_coverage(&mut instance)?;
        Ok(result?)
    }

    pub fn bank_query(&mut self, bank_query_: &[u8]) -> Result<Binary, Error> {
        let bank_query: BankQuery =
            from_binary(&Binary::from(bank_query_)).map_err(Error::format_error)?;
        self.states.write().unwrap().bank_query(&bank_query)
    }

    fn new_mock(
        &self,
        contract_storage: &Arc<RwLock<ContractStorage>>,
    ) -> Result<RpcBackend, Error> {
        let states = self.states.read().unwrap();
        let canonical_address_length = states.canonical_address_length;
        let bech32_prefix = states.bech32_prefix.to_string();
        Ok(Backend {
            storage: self.mock_storage(contract_storage)?,
            // is this correct?
            api: RpcMockApi::new(canonical_address_length, bech32_prefix.as_str())?,
            querier: RpcMockQuerier::new(&self.states, &self.debug_log),
        })
    }

    fn env(&self, contract_addr: &Addr) -> Result<Env, Error> {
        let states = self.states.read().unwrap();
        let block_number = states.block_number;
        let block_timestamp = states.block_timestamp;
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

    fn mock_storage(
        &self,
        contract_storage: &Arc<RwLock<ContractStorage>>,
    ) -> Result<RpcMockStorage, Error> {
        let storage = RpcMockStorage::new(contract_storage);
        Ok(storage)
    }

    /// modify block number
    pub fn cheat_block_number(&mut self, new_number: u64) -> Result<(), Error> {
        self.states.write().unwrap().block_number = new_number;
        Ok(())
    }

    /// modify block timestamp
    pub fn cheat_block_timestamp(&mut self, new_timestamp: Timestamp) -> Result<(), Error> {
        self.states.write().unwrap().block_timestamp = new_timestamp;
        Ok(())
    }

    /// modify bank balance
    pub fn cheat_bank_balance(
        &mut self,
        address: &Addr,
        denom: &str,
        new_balance: u128,
    ) -> Result<(), Error> {
        self.states
            .write()
            .unwrap()
            .set_balance(address, denom, Uint128::new(new_balance))?;
        Ok(())
    }

    /// modify code
    pub fn cheat_code(&mut self, contract_addr: &Addr, new_code: &[u8]) -> Result<(), Error> {
        self.fetch_contract_state(contract_addr)?;

        let old_contract_state = self
            .states
            .read()
            .unwrap()
            .contract_state_get(contract_addr)
            .unwrap()
            .clone();
        let mut new_contract_state = old_contract_state.clone();
        new_contract_state.code = new_code.to_vec();
        self.states
            .write()
            .unwrap()
            .contract_state_insert(contract_addr.clone(), new_contract_state);
        // try creating an instance to check if provided wasm is valid
        self.create_instance(contract_addr).map_err(|e| {
            self.states
                .write()
                .unwrap()
                .contract_state_insert(contract_addr.clone(), old_contract_state);
            e
        })?;
        Ok(())
    }

    /// modify message sender
    pub fn cheat_message_sender(&mut self, my_addr: &Addr) -> Result<(), Error> {
        self.sender = my_addr.to_string();
        Ok(())
    }

    /// modify storage of a contract
    pub fn cheat_storage(
        &mut self,
        contract_addr: &Addr,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), Error> {
        self.fetch_contract_state(contract_addr)?;
        let mut states = self.states.write().unwrap();
        let contract_storage = states.contract_state_get_mut(contract_addr).unwrap();
        contract_storage
            .storage
            .write()
            .unwrap()
            .insert(key.to_vec(), value.to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod test {

    use cosmwasm_std::{from_binary, to_binary, Addr, BalanceResponse, BankQuery, Coin, Uint128};
    use serde_json::json;
    use std::str::FromStr;

    use crate::{fork::debug_log::DebugLogEntry, fork::model::Model};

    const MALAGA_RPC_URL: &str = "https://rpc.malaga-420.cosmwasm.com:443";
    const MALAGA_BLOCK_NUMBER: u64 = 2326474;
    const FACTORY_ADDRESS_MALAGA: &str =
        "wasm1hczjykytm4suw4586j5v42qft60gc4j307gf7cxuazfg7jxt4h4sjvp7rx";
    const PAIR_ADDRESS_MALAGA: &str =
        "wasm15le5evw4regnwf9lrjnpakr2075fcyp4n4yzpelvqcuevzkw2lss46hslz";
    const TOKEN_ADDRESS_MALAGA: &str =
        "wasm124v54ngky9wxhx87t252x4xfgujmdsu7uhjdugtkkqt39nld0e6st7e64h";
    const VAULT_ADDRESS: &str = "wasm1fedmcgtsvmymyr6jssgar0h7uhhcuxhr7ygjjw5q2epgzef3jy0svcr5jx";
    const VAULT_ROUTER_ADDRESS: &str =
        "wasm1xp8prmlsx9erdkrk43qjtrw54755zwm9f4x52m8k3an6jgcaldpqpmsd23";

    #[test]
    fn test_swap_basic_testnet() {
        use serde_json::Value::Null;
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER), "wasm").unwrap();
        let pair_address = Addr::unchecked(PAIR_ADDRESS_MALAGA);
        let token_address = Addr::unchecked(TOKEN_ADDRESS_MALAGA);
        let my_address = model.sender.clone();

        let swap_msg_json = json!({
            "swap": {
            "offer_asset": {
                "info": { "native_token": { "denom": "umlg" } },
                "amount": "10"
            },
            "belief_price": Null,
            "max_spread": Null,
            "to": Null
            }
        });
        let swap_msg = serde_json::to_string(&swap_msg_json).unwrap();
        let funds = vec![Coin {
            denom: "umlg".to_string(),
            amount: Uint128::new(10),
        }];

        // balance before the swap
        let query_balance_msg_json = json!({
            "balance": {"address": my_address, }
        });
        let query_balance_msg = serde_json::to_string(&query_balance_msg_json).unwrap();
        let resp = model
            .wasm_query(&token_address, query_balance_msg.as_bytes())
            .unwrap();
        let resp_json: serde_json::Value = serde_json::from_slice(resp.as_slice()).unwrap();
        let token_balance_before = u128::from_str(resp_json["balance"].as_str().unwrap()).unwrap();
        let bank_query = BankQuery::Balance {
            address: my_address,
            denom: "umlg".to_string(),
        };
        let resp = model
            .bank_query(to_binary(&bank_query).unwrap().as_slice())
            .unwrap();
        let resp_bank: BalanceResponse = serde_json::from_slice(resp.as_slice()).unwrap();
        let umlg_balance_before: u128 = resp_bank.amount.amount.into();
        let prev_block_num = model.states.read().unwrap().block_number;

        // execute the swap transaction
        let _ = model
            .execute(&pair_address, swap_msg.as_bytes(), &funds)
            .unwrap();

        // check the results
        // block number incremented
        assert_eq!(
            model.states.read().unwrap().block_number,
            prev_block_num + 1
        );

        let resp = model
            .wasm_query(&token_address, query_balance_msg.as_bytes())
            .unwrap();
        let resp_json: serde_json::Value = serde_json::from_slice(resp.as_slice()).unwrap();
        let token_balance_after = u128::from_str(resp_json["balance"].as_str().unwrap()).unwrap();
        let resp = model
            .bank_query(to_binary(&bank_query).unwrap().as_slice())
            .unwrap();
        let resp_bank: BalanceResponse = serde_json::from_slice(resp.as_slice()).unwrap();
        let umlg_balance_after: u128 = resp_bank.amount.amount.into();

        // token and umlg balance as expected
        assert_eq!(token_balance_after - token_balance_before, 9);
        assert_eq!(umlg_balance_before - umlg_balance_after, 10);
    }

    #[test]
    fn test_flashloan() {
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER), "wasm").unwrap();
        let _vault_address = Addr::unchecked(VAULT_ADDRESS);
        let vault_router_address = Addr::unchecked(VAULT_ROUTER_ADDRESS);
        let _my_address = model.sender.clone();

        let loan_msg_json = json!({
            "flash_loan": {
                "assets": [{
                    "info": { "native_token": { "denom": "umlg" } },
                    "amount": "10"
                }],
                "msgs": [],
            }
        });
        let loan_msg = serde_json::to_string(&loan_msg_json).unwrap();
        let prev_block_num = model.states.read().unwrap().block_number;
        // execute the swap transaction
        let log = model
            .execute(&vault_router_address, loan_msg.as_bytes(), &[])
            .unwrap();

        assert_eq!(
            model.states.read().unwrap().block_number,
            prev_block_num + 1
        );
        assert_eq!(log.err_msg, None);
    }

    #[test]
    fn test_storage_write() {
        use test_contract::msg::ExecuteMsg;
        // test if querier can view writes to the current contract
        let wasm_code = include_bytes!(concat!(
            env!("OUT_DIR"),
            "/wasm32-unknown-unknown/release/test_contract.wasm"
        ));
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER), "wasm").unwrap();
        let pair_address = Addr::unchecked(PAIR_ADDRESS_MALAGA);
        model.cheat_code(&pair_address, wasm_code).unwrap();
        let msg = to_binary(&ExecuteMsg::TestQuerySelf {}).unwrap();
        let res = model.execute(&pair_address, msg.as_slice(), &[]).unwrap();
        for log in res.logs {
            for event in log.events {
                if event.ty == "read_number" {
                    assert_eq!(event.attributes[0].value.as_str(), "2");
                }
            }
        }
    }

    #[test]
    fn test_atomicity() {
        use test_contract::msg::{ExecuteMsg, QueryMsg, ReadNumberResponse};
        // test if querier can view writes to the current contract
        let wasm_code = include_bytes!(concat!(
            env!("OUT_DIR"),
            "/wasm32-unknown-unknown/release/test_contract.wasm"
        ));
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER), "wasm").unwrap();
        let pair_address = Addr::unchecked(PAIR_ADDRESS_MALAGA);
        model.cheat_code(&pair_address, wasm_code).unwrap();

        // set NUMBER to 2
        let msg = to_binary(&ExecuteMsg::TestQuerySelf {}).unwrap();
        let _ = model.execute(&pair_address, msg.as_slice(), &[]).unwrap();

        // query value of NUMBER
        let msg = to_binary(&QueryMsg::ReadNumber {}).unwrap();
        let query_res1: ReadNumberResponse =
            from_binary(&model.wasm_query(&pair_address, msg.as_slice()).unwrap()).unwrap();

        // run failing execute()
        let msg = to_binary(&ExecuteMsg::TestAtomic {}).unwrap();
        let _ = model.execute(&pair_address, msg.as_slice(), &[]).unwrap();

        // query value of NUMBER again, it should be same as previous value
        let msg = to_binary(&QueryMsg::ReadNumber {}).unwrap();
        let query_res2: ReadNumberResponse =
            from_binary(&model.wasm_query(&pair_address, msg.as_slice()).unwrap()).unwrap();
        assert_eq!(query_res1.value, query_res2.value);
    }

    #[test]
    fn test_query() {
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER), "wasm").unwrap();
        let normal_query = to_binary(&BankQuery::Balance {
            address: PAIR_ADDRESS_MALAGA.to_string(),
            denom: "umlg".to_string(),
        })
        .unwrap();
        let query_result1 =
            String::from_utf8(model.bank_query(normal_query.as_slice()).unwrap().to_vec()).unwrap();
        let abnormal_query = to_binary(&BankQuery::Balance {
            address: PAIR_ADDRESS_MALAGA.to_string(),
            denom: "umlg".to_string(),
        })
        .unwrap();
        let query_result2 = String::from_utf8(
            model
                .bank_query(abnormal_query.as_slice())
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        println!("{}", query_result1);
        println!("{}", query_result2);
    }

    fn get_contract_address_from_log(logs: &[DebugLogEntry]) -> Option<String> {
        for log in logs.iter() {
            for event in log.events.iter() {
                for attribute in event.attributes.iter() {
                    if attribute.key.as_str() == "_contract_address" {
                        return Some(attribute.value.clone());
                    }
                }
            }
        }
        None
    }

    #[test]
    fn test_add_custom_code() {
        use test_contract::msg::{InstantiateMsg, QueryMsg, ReadNumberResponse};
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER), "wasm").unwrap();
        let code = include_bytes!(concat!(
            env!("OUT_DIR"),
            "/wasm32-unknown-unknown/release/test_contract.wasm"
        ));
        model.add_custom_code(1337, code).unwrap();
        let msg = to_binary(&InstantiateMsg {}).unwrap();
        let funds = vec![];
        let debug_log = model.instantiate(1337, msg.as_slice(), &funds).unwrap();
        let contract_address =
            Addr::unchecked(get_contract_address_from_log(&debug_log.logs).unwrap());
        let msg = to_binary(&QueryMsg::ReadNumber {}).unwrap();
        let query_res: ReadNumberResponse =
            from_binary(&model.wasm_query(&contract_address, msg.as_slice()).unwrap()).unwrap();
        assert_eq!(query_res.value, 1);
    }

    #[test]
    fn test_call_trace() {
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER), "wasm").unwrap();
        let create_pair_msg = json!({
            "create_pair": {
                "asset_infos": [
                  {
                    "native_token": {
                      "denom": "umlg"
                    }
                  },
                  {
                    "native_token": {
                      "denom": "umlg"
                    }
                  }
                ]
              }
        })
        .to_string();
        let res = model
            .execute(
                &Addr::unchecked(FACTORY_ADDRESS_MALAGA),
                create_pair_msg.as_bytes(),
                &vec![],
            )
            .unwrap();

        // first pair creation results in an error, due to same native token error
        assert_eq!(res.call_trace.call_graph.get(&0).unwrap().len(), 1);
    }
}
