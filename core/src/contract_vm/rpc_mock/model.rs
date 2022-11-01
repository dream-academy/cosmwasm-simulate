use crate::contract_vm::rpc_mock::api::canonical_to_human;
use crate::contract_vm::rpc_mock::{
    Bank, CwRpcClient, DebugLog, RpcContractInstance, RpcMockApi, RpcMockQuerier, RpcMockStorage,
};
use crate::contract_vm::Error;

use cosmwasm_std::{
    Addr, BankMsg, Binary, Coin, ContractInfo, ContractResult, CosmosMsg, Env, Reply, ReplyOn,
    Response, SubMsgResponse, SubMsgResult, Timestamp, Uint128, WasmMsg, WasmQuery,
};
use cosmwasm_vm::{Backend, InstanceOptions, Storage};
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::mem;
use std::sync::{Arc, Mutex};

pub type RpcBackend = Backend<RpcMockApi, RpcMockStorage, RpcMockQuerier>;

pub struct Model {
    instances: Arc<DashMap<Addr, RpcContractInstance>>,
    bank: Arc<Mutex<Bank>>,
    client: Arc<Mutex<CwRpcClient>>,
    // similar to tx.origin of solidity
    sender: String,
    // used to generate addresses in instantiate
    code_id_counters: HashMap<u64, u64>,

    // fields related to blockchain environment
    block_number: u64,
    block_timestamp: Timestamp,
    chain_id: String,
    canonical_address_length: usize,
    bech32_prefix: String,
}

const BLOCK_EPOCH: u64 = 1_000_000_000;
const WASM_MAGIC: [u8; 4] = [0, 97, 115, 109];
const GZIP_MAGIC: [u8; 4] = [0, 0, 0, 0];
const BASE_EOA: &str = "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj";

fn maybe_unzip(input: Vec<u8>) -> Result<Vec<u8>, Error> {
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
    /// duplicates self
    fn clone(&self) -> Self {
        let instances = DashMap::new();
        for it in self.instances.iter_mut() {
            instances.insert(it.key().clone(), it.value().clone());
        }
        let bank = Arc::new(Mutex::new(self.bank.lock().unwrap().clone()));
        let client = Arc::new(Mutex::new(self.client.lock().unwrap().clone()));
        Model {
            instances: Arc::new(instances),
            bank,
            client,
            // similar to tx.origin of solidity
            sender: self.sender.clone(),
            // used to generate addresses in instantiate
            code_id_counters: self.code_id_counters.clone(),

            // fields related to blockchain environment
            block_number: self.block_number,
            block_timestamp: self.block_timestamp.clone(),
            chain_id: self.chain_id.clone(),
            canonical_address_length: self.canonical_address_length,
            bech32_prefix: self.bech32_prefix.clone(),
        }
    }
}

impl Model {
    pub fn new(
        rpc_url: &str,
        block_number: Option<u64>,
        bech32_prefix: &str,
    ) -> Result<Self, Error> {
        let client = CwRpcClient::new(rpc_url, block_number)?;
        let block_number = client.block_number();
        let block_timestamp = client.timestamp()?;
        let chain_id = client.chain_id()?;
        let client = Arc::new(Mutex::new(client));
        Ok(Model {
            instances: Arc::new(DashMap::new()),
            bank: Arc::new(Mutex::new(Bank::new(&client)?)),
            client,
            sender: BASE_EOA.to_string(),
            code_id_counters: HashMap::new(),

            block_number,
            block_timestamp,
            chain_id,
            canonical_address_length: 32,
            bech32_prefix: bech32_prefix.to_string(),
        })
    }

    fn create_instance(&mut self, contract_addr: &Addr) -> Result<(), Error> {
        let deps = self.new_mock(&contract_addr)?;
        let options = InstanceOptions {
            gas_limit: u64::MAX,
            print_debug: false,
        };
        let mut client = self.client.lock().unwrap();
        let contract_info = client.query_wasm_contract_info(contract_addr.as_str())?;
        let wasm_code = maybe_unzip(client.query_wasm_contract_code(contract_info.code_id)?)?;
        let wasm_instance =
            match cosmwasm_vm::Instance::from_code(wasm_code.as_slice(), deps, options, None) {
                Err(e) => {
                    return Err(Error::vm_error(e));
                }
                Ok(i) => i,
            };
        let instance = RpcContractInstance::new(&contract_addr, wasm_instance);
        self.instances.insert(contract_addr.clone(), instance);
        Ok(())
    }

    fn generate_address(&mut self, code_id: u64) -> Result<Addr, Error> {
        let code_id_counter = self.code_id_counters.entry(code_id).or_insert(0);
        let seed = format!("seeeed_{}_{}", code_id, *code_id_counter);
        *code_id_counter += 1;
        let mut hasher = Sha256::new();
        hasher.update(seed);
        let bytes = hasher.finalize();
        let addr = canonical_to_human(
            bytes.as_slice(),
            &self.bech32_prefix,
            self.canonical_address_length,
        )
        .map_err(|e| Error::serialization_error(&e))?;
        Ok(Addr::unchecked(addr))
    }

    fn handle_response(
        &mut self,
        origin: &Addr,
        response: &Response,
        debug_log: &mut DebugLog,
    ) -> Result<ContractResult<Response>, Error> {
        // last_response is the response of the latest execution
        // If there are no submessages, this will be returned. Otherwise, response from the submessages will be returned
        if response.messages.len() == 0 {
            return Ok(ContractResult::Ok(response.clone()));
        }
        // this will be overwritten at least once
        let mut last_response = ContractResult::Ok(Response::new());
        // otherwise, execute the submessages
        for sub_msg in response.messages.iter() {
            let (response, target_addr) = match &sub_msg.msg {
                CosmosMsg::Wasm(wasm_msg) => match wasm_msg {
                    WasmMsg::Instantiate {
                        admin,
                        code_id,
                        msg,
                        funds,
                        label: _,
                    } => {
                        match admin {
                            Some(_) => unimplemented!(),
                            None => {}
                        }
                        // generate contract address automatically
                        let contract_addr = self.generate_address(*code_id)?;
                        (
                            self.instantiate_inner(
                                &contract_addr,
                                *code_id,
                                &origin,
                                msg,
                                funds,
                                debug_log,
                            )?,
                            contract_addr,
                        )
                    }
                    WasmMsg::Execute {
                        contract_addr: target_addr,
                        msg,
                        funds,
                    } => {
                        let target_addr = Addr::unchecked(target_addr);
                        (
                            self.execute_inner(
                                &target_addr,
                                &origin,
                                msg.as_slice(),
                                funds,
                                debug_log,
                            )?,
                            target_addr,
                        )
                    }
                    _ => unimplemented!(),
                },
                CosmosMsg::Bank(bank_msg) => {
                    // if bank fails, revert the entire transaction
                    let mut bank = self.bank.lock().unwrap();
                    match bank.execute(&origin, &bank_msg)? {
                        ContractResult::Ok(r) => {
                            debug_log.append_log(&r);
                            last_response = ContractResult::Ok(r);
                        }
                        ContractResult::Err(e) => {
                            debug_log.set_err_msg(&e);
                            return Ok(ContractResult::Err(e));
                        }
                    };
                    continue;
                }
                _ => unimplemented!(),
            };
            let do_reply = match &sub_msg.reply_on {
                ReplyOn::Always => true,
                ReplyOn::Success => response.is_ok(),
                ReplyOn::Error => response.is_err(),
                ReplyOn::Never => false,
            };
            // call reply(), and recursively handle response
            last_response = if do_reply {
                let env = self.env(&target_addr)?;
                let reply = Reply {
                    id: sub_msg.id,
                    result: match response {
                        ContractResult::Ok(r) => SubMsgResult::Ok(SubMsgResponse {
                            events: r.events,
                            data: r.data,
                        }),
                        ContractResult::Err(e) => SubMsgResult::Err(e),
                    },
                };
                if !self.instances.contains_key(origin) {
                    self.create_instance(origin)?;
                }
                let mut instance = self.instances.get_mut(origin).unwrap();
                let maybe_response = instance.reply(&env, &reply)?;
                drop(instance);

                if maybe_response.is_err() {
                    // propagate error. instance.reply need not error handling
                    return Ok(maybe_response);
                } else {
                    let response = maybe_response.unwrap();
                    debug_log.append_log(&response);
                    self.handle_response(origin, &response, debug_log)?
                }
            }
            // if reply is not called, but the current result is an error, propagate the error
            else if response.is_err() {
                return Ok(ContractResult::Err(response.unwrap_err()));
            }
            // otherwise, recursively handle the submessages
            else {
                self.handle_response(origin, &response.unwrap(), debug_log)?
            };
        }
        Ok(last_response)
    }

    /// TODO: fix instantiate so that it generates the address automatically
    pub fn instantiate(
        &mut self,
        code_id: u64,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<DebugLog, Error> {
        let sender = self.sender.clone();
        let mut debug_log = DebugLog::new();
        let state_copy = self.clone();
        let contract_addr = self.generate_address(code_id)?;
        self.instantiate_inner(
            &contract_addr,
            code_id,
            &Addr::unchecked(sender),
            msg,
            funds,
            &mut debug_log,
        )
        .map_err(|e| {
            // revert entire state
            let _ = mem::replace(self, state_copy);
            e
        })?;
        self.update_block();
        Ok(debug_log)
    }

    fn instantiate_inner(
        &mut self,
        // this argument should be removed someday
        contract_addr: &Addr,
        code_id: u64,
        sender: &Addr,
        msg: &[u8],
        funds: &[Coin],
        debug_log: &mut DebugLog,
    ) -> Result<ContractResult<Response>, Error> {
        // transfer coins
        let mut bank = self.bank.lock().unwrap();
        let bank_msg = BankMsg::Send {
            to_address: contract_addr.to_string(),
            amount: funds.to_vec(),
        };
        match bank.execute(sender, &bank_msg)? {
            ContractResult::Ok(r) => {
                debug_log.append_log(&r);
            }
            ContractResult::Err(e) => {
                debug_log.set_err_msg(&e);
                return Ok(ContractResult::Err(e));
            }
        };
        drop(bank);

        let deps = self.new_mock(contract_addr)?;
        let options = InstanceOptions {
            gas_limit: u64::MAX,
            print_debug: false,
        };
        let mut client = self.client.lock().unwrap();
        let wasm_code = maybe_unzip(client.query_wasm_contract_code(code_id)?)?;
        drop(client);

        let wasm_instance =
            match cosmwasm_vm::Instance::from_code(wasm_code.as_slice(), deps, options, None) {
                Err(e) => {
                    return Err(Error::vm_error(e));
                }
                Ok(i) => i,
            };
        let mut instance = RpcContractInstance::new(contract_addr, wasm_instance);
        let env = self.env(contract_addr)?;
        // propagate contract error downwards
        let response = match instance.instantiate(&env, msg, &sender, funds)? {
            ContractResult::Ok(r) => {
                debug_log.append_log(&r);
                r
            }
            ContractResult::Err(e) => {
                debug_log.set_err_msg(&e);
                return Ok(ContractResult::Err(e));
            }
        };
        let response = self.handle_response(contract_addr, &response, debug_log)?;
        self.instances.insert(contract_addr.clone(), instance);
        Ok(response)
    }

    pub fn execute(
        &mut self,
        contract_addr: &Addr,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<DebugLog, Error> {
        let mut debug_log = DebugLog::new();
        let sender = self.sender.clone();
        let state_copy = self.clone();
        self.execute_inner(
            contract_addr,
            &Addr::unchecked(sender),
            msg,
            funds,
            &mut debug_log,
        )
        .map_err(|e| {
            // revert entire state
            let _ = mem::replace(self, state_copy);
            e
        })?;
        self.update_block();
        Ok(debug_log)
    }

    fn execute_inner(
        &mut self,
        contract_addr: &Addr,
        sender: &Addr,
        msg: &[u8],
        funds: &[Coin],
        debug_log: &mut DebugLog,
    ) -> Result<ContractResult<Response>, Error> {
        let env = self.env(contract_addr)?;

        // create instance if instance is not materialized
        if !self.instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }

        // transfer coins
        let mut bank = self.bank.lock().unwrap();
        let bank_msg = BankMsg::Send {
            to_address: contract_addr.to_string(),
            amount: funds.to_vec(),
        };
        match bank.execute(sender, &bank_msg)? {
            ContractResult::Ok(r) => {
                debug_log.append_log(&r);
            }
            ContractResult::Err(e) => {
                debug_log.set_err_msg(&e);
                return Ok(ContractResult::Err(e));
            }
        };
        drop(bank);

        // execute contract code
        let mut instance = self.instances.get_mut(contract_addr).unwrap();
        // propagate contract error downwards
        let response = match instance.execute(&env, msg, &sender, funds)? {
            ContractResult::Ok(r) => {
                debug_log.append_log(&r);
                r
            }
            ContractResult::Err(e) => {
                debug_log.set_err_msg(&e);
                return Ok(ContractResult::Err(e));
            }
        };
        drop(instance);
        self.handle_response(contract_addr, &response, debug_log)
    }

    /// for now, only support WASM queries
    pub fn query_wasm(&mut self, contract_addr: &Addr, msg: &[u8]) -> Result<Binary, Error> {
        let env = self.env(contract_addr)?;
        if !self.instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }
        let mut instance = self.instances.get_mut(contract_addr).unwrap();
        let wasm_query = WasmQuery::Smart {
            contract_addr: contract_addr.to_string(),
            msg: Binary::from(msg),
        };
        // TODO: fix this, propagate contract error down
        instance.query(&env, &wasm_query)
    }

    /// emulate blockchain block creation
    /// increment block number by 1
    /// increment timestamp by a constant
    fn update_block(&mut self) {
        self.block_number += 1;
        self.block_timestamp.plus_nanos(BLOCK_EPOCH);
    }

    fn new_mock(&self, contract_addr: &Addr) -> Result<RpcBackend, Error> {
        Ok(Backend {
            storage: self.mock_storage(contract_addr)?,
            // is this correct?
            api: RpcMockApi::new(self.canonical_address_length, self.bech32_prefix.as_str())?,
            querier: RpcMockQuerier::new(&self.client, &self.bank, &self.instances),
        })
    }

    fn env(&self, contract_addr: &Addr) -> Result<Env, Error> {
        Ok(Env {
            block: cosmwasm_std::BlockInfo {
                height: self.block_number,
                time: self.block_timestamp.clone(),
                chain_id: self.chain_id.clone(),
            },
            // assumption: all blocks have only 1 transaction
            transaction: Some(cosmwasm_std::TransactionInfo { index: 0 }),
            // I don't really know what this is for, so for now, set it to the target contract address
            contract: ContractInfo {
                address: contract_addr.clone(),
            },
        })
    }

    fn mock_storage(&self, contract_addr: &Addr) -> Result<RpcMockStorage, Error> {
        let mut storage = RpcMockStorage::new();
        let mut client = self.client.lock().unwrap();
        let states = client.query_wasm_contract_all(contract_addr.as_str())?;
        for (k, v) in states {
            storage
                .set(k.as_slice(), v.as_slice())
                .0
                .map_err(|x| Error::vm_error(x))?;
        }
        Ok(storage)
    }

    /// modify block number
    pub fn cheat_block_number(&mut self, new_number: u64) -> Result<(), Error> {
        self.block_number = new_number;
        Ok(())
    }

    /// modify block timestamp
    pub fn cheat_block_timestamp(&mut self, new_timestamp: Timestamp) -> Result<(), Error> {
        self.block_timestamp = new_timestamp;
        Ok(())
    }

    /// modify bank balance
    pub fn cheat_bank_balance(
        &mut self,
        address: &Addr,
        denom: &str,
        new_balance: u128,
    ) -> Result<(), Error> {
        let mut bank = self.bank.lock().unwrap();
        bank.set_balance(address, denom, Uint128::new(new_balance))?;
        Ok(())
    }

    /// modify code
    pub fn cheat_code(&mut self, contract_addr: &Addr, new_code: &[u8]) -> Result<(), Error> {
        if !self.instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }
        let (_, instance) = self.instances.remove(contract_addr).unwrap();
        let deps = instance.recycle();
        let options = InstanceOptions {
            gas_limit: u64::MAX,
            print_debug: false,
        };
        let instance_inner = match cosmwasm_vm::Instance::from_code(new_code, deps, options, None) {
            Err(e) => {
                return Err(Error::vm_error(e));
            }
            Ok(i) => i,
        };
        let instance = RpcContractInstance::new(&contract_addr, instance_inner);
        self.instances.insert(contract_addr.clone(), instance);
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
        if !self.instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }
        let mut instance = self.instances.get_mut(contract_addr).unwrap();
        instance.storage_write(key, value)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {

    use cosmwasm_std::{Addr, BalanceResponse, BankQuery, Coin, Uint128};
    use serde_json::json;
    use std::str::FromStr;

    use crate::contract_vm::rpc_mock::model::Model;

    const MALAGA_RPC_URL: &'static str = "https://rpc.malaga-420.cosmwasm.com:443";
    const MALAGA_BLOCK_NUMBER: u64 = 2326474;
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
            "balance": {"address": my_address.clone(), }
        });
        let query_balance_msg = serde_json::to_string(&query_balance_msg_json).unwrap();
        let resp = model
            .query_wasm(&token_address, query_balance_msg.as_bytes())
            .unwrap();
        let resp_json: serde_json::Value = serde_json::from_slice(resp.as_slice()).unwrap();
        let token_balance_before = u128::from_str(resp_json["balance"].as_str().unwrap()).unwrap();
        let bank_query = BankQuery::Balance {
            address: my_address.clone(),
            denom: "umlg".to_string(),
        };
        let resp = model.bank.lock().unwrap().query(&bank_query).unwrap();
        let resp_bank: BalanceResponse = serde_json::from_slice(resp.as_slice()).unwrap();
        let umlg_balance_before: u128 = resp_bank.amount.amount.into();
        let prev_block_num = model.block_number;

        // execute the swap transaction
        let _ = model
            .execute(&pair_address, swap_msg.as_bytes(), &funds)
            .unwrap();

        // check the results
        // block number incremented
        assert_eq!(model.block_number, prev_block_num + 1);

        let resp = model
            .query_wasm(&token_address, query_balance_msg.as_bytes())
            .unwrap();
        let resp_json: serde_json::Value = serde_json::from_slice(resp.as_slice()).unwrap();
        let token_balance_after = u128::from_str(resp_json["balance"].as_str().unwrap()).unwrap();
        let resp = model.bank.lock().unwrap().query(&bank_query).unwrap();
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
        let prev_block_num = model.block_number;
        // execute the swap transaction
        let log = model
            .execute(&vault_router_address, loan_msg.as_bytes(), &vec![])
            .unwrap();

        assert_eq!(model.block_number, prev_block_num + 1);
        assert_eq!(log.err_msg, None);
    }
}
