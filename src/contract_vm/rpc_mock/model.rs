use crate::contract_vm::rpc_mock::{
    Bank, CwRpcClient, RpcContractInstance, RpcMockApi, RpcMockQuerier, RpcMockStorage,
};
use crate::contract_vm::Error;

use cosmwasm_std::{
    from_slice, Addr, BankMsg, Binary, Coin, ContractInfo, CosmosMsg, Env, Response, Timestamp,
    Uint128, WasmMsg, WasmQuery,
};
use cosmwasm_vm::{call_instantiate, Backend, InstanceOptions, Storage};
use std::cell::{RefCell, UnsafeCell};
use std::collections::HashMap;
use std::rc::Rc;

pub type RpcBackend = Backend<RpcMockApi, RpcMockStorage, RpcMockQuerier>;

pub struct Model {
    instances: Rc<UnsafeCell<HashMap<Addr, RpcContractInstance>>>,
    bank: Rc<RefCell<Bank>>,
    client: Rc<RefCell<CwRpcClient>>,
    // similar to tx.origin of solidity
    eoa: String,

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

impl Model {
    pub fn new(rpc_url: &str, block_number: Option<u64>) -> Result<Self, Error> {
        let client = CwRpcClient::new(rpc_url, block_number)?;
        let block_number = client.block_number();
        let block_timestamp = client.timestamp()?;
        let chain_id = client.chain_id()?;
        let client = Rc::new(RefCell::new(client));
        Ok(Model {
            instances: Rc::new(UnsafeCell::new(HashMap::new())),
            bank: Rc::new(RefCell::new(Bank::new(&client)?)),
            client,
            eoa: BASE_EOA.to_string(),

            block_number,
            block_timestamp,
            chain_id,
            canonical_address_length: 32,
            bech32_prefix: "wasm".to_string(),
        })
    }

    fn create_instance(&mut self, contract_addr: &Addr) -> Result<(), Error> {
        let deps = self.new_mock(&contract_addr)?;
        let options = InstanceOptions {
            gas_limit: u64::MAX,
            print_debug: false,
        };
        let mut client = self.client.borrow_mut();
        let contract_info = client.query_wasm_contract_info(contract_addr.as_str())?;
        let wasm_code = maybe_unzip(client.query_wasm_contract_code(contract_info.code_id)?)?;
        let wasm_instance =
            match cosmwasm_vm::Instance::from_code(wasm_code.as_slice(), deps, options, None) {
                Err(e) => {
                    return Err(Error::vm_error(e));
                }
                Ok(i) => i,
            };
        let instance = RpcContractInstance::make_instance(&contract_addr, wasm_instance);
        let instances = unsafe { self.instances.get().as_mut().unwrap() };
        instances.insert(contract_addr.clone(), instance);
        Ok(())
    }

    fn handle_response(&mut self, origin: &Addr, response: &Response) -> Result<(), Error> {
        for resp in response.messages.iter() {
            match &resp.msg {
                CosmosMsg::Wasm(wasm_msg) => match wasm_msg {
                    WasmMsg::Instantiate {
                        admin,
                        code_id,
                        msg,
                        funds,
                        label,
                    } => {
                        unimplemented!()
                    }
                    WasmMsg::Execute {
                        contract_addr: target_addr,
                        msg,
                        funds,
                    } => {
                        let target_addr = Addr::unchecked(target_addr);
                        self.execute_inner(
                            &Addr::unchecked(target_addr),
                            origin,
                            msg.as_slice(),
                            funds,
                        )?;
                    }
                    _ => unimplemented!(),
                },
                CosmosMsg::Bank(bank_msg) => {
                    self.bank.borrow_mut().execute(origin, bank_msg)?;
                }
                _ => unimplemented!(),
            }
        }
        Ok(())
    }

    /// TODO: fix instantiate so that it generates the address automatically
    pub fn instantiate(
        &mut self,
        contract_addr: &Addr,
        code_id: u64,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<(), Error> {
        let deps = self.new_mock(contract_addr)?;
        let options = InstanceOptions {
            gas_limit: u64::MAX,
            print_debug: false,
        };
        let mut client = self.client.borrow_mut();
        let wasm_code = maybe_unzip(client.query_wasm_contract_code(code_id)?)?;
        drop(client);
        let wasm_instance =
            match cosmwasm_vm::Instance::from_code(wasm_code.as_slice(), deps, options, None) {
                Err(e) => {
                    return Err(Error::vm_error(e));
                }
                Ok(i) => i,
            };
        let mut instance = RpcContractInstance::make_instance(contract_addr, wasm_instance);
        let env = self.env(contract_addr)?;
        let sender = Addr::unchecked(self.eoa.clone());
        let response = instance.instantiate(&env, msg, &sender, funds)?;
        self.handle_response(contract_addr, &response)?;
        let instances = unsafe { self.instances.get().as_mut().unwrap() };
        instances.insert(contract_addr.clone(), instance);
        Ok(())
    }

    pub fn execute(
        &mut self,
        contract_addr: &Addr,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<(), Error> {
        let eoa = self.eoa.clone();
        self.execute_inner(contract_addr, &Addr::unchecked(eoa), msg, funds)?;
        self.update_block();
        Ok(())
    }

    fn execute_inner(
        &mut self,
        contract_addr: &Addr,
        sender: &Addr,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<(), Error> {
        let env = self.env(contract_addr)?;

        // create instance if instance is not materialized
        let instances = unsafe { self.instances.get().as_mut().unwrap() };
        if !instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }

        // transfer coins
        let mut bank = self.bank.borrow_mut();
        let bank_msg = BankMsg::Send {
            to_address: contract_addr.to_string(),
            amount: funds.to_vec(),
        };
        bank.execute(sender, &bank_msg)?;
        drop(bank);

        // execute contract code
        let instance = instances.get_mut(contract_addr).unwrap();
        let response = instance.execute(&env, msg, &sender, funds)?;
        self.handle_response(contract_addr, &response)?;
        Ok(())
    }

    /// for now, only support WASM queries
    pub fn query_wasm(&mut self, contract_addr: &Addr, msg: &[u8]) -> Result<Binary, Error> {
        let env = self.env(contract_addr)?;
        let instances = unsafe { self.instances.get().as_mut().unwrap() };
        if !instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }
        let instance = instances.get_mut(contract_addr).unwrap();
        let wasm_query = WasmQuery::Smart {
            contract_addr: contract_addr.to_string(),
            msg: Binary::from(msg),
        };
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
        let mut client = self.client.borrow_mut();
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
        let mut bank = self.bank.borrow_mut();
        bank.set_balance(address, denom, Uint128::new(new_balance))?;
        Ok(())
    }

    /// modify code
    pub fn cheat_code(&mut self, contract_addr: &Addr, new_code: &[u8]) -> Result<(), Error> {
        let instances = unsafe { self.instances.get().as_mut().unwrap() };
        if !instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }
        let instance = instances.remove(contract_addr).unwrap();
        let deps = instance.recycle();
        let options = InstanceOptions {
            gas_limit: u64::MAX,
            print_debug: false,
        };
        let wasm_instance = match cosmwasm_vm::Instance::from_code(new_code, deps, options, None) {
            Err(e) => {
                return Err(Error::vm_error(e));
            }
            Ok(i) => i,
        };
        let instance = RpcContractInstance::make_instance(&contract_addr, wasm_instance);
        instances.insert(contract_addr.clone(), instance);
        Ok(())
    }

    /// modify message sender
    pub fn cheat_message_sender(&mut self, my_addr: &Addr) -> Result<(), Error> {
        self.eoa = my_addr.to_string();
        Ok(())
    }

    /// modify storage of a contract
    pub fn cheat_storage(
        &mut self,
        contract_addr: &Addr,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), Error> {
        let instances = unsafe { self.instances.get().as_mut().unwrap() };
        if !instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }
        let mut instance = instances.remove(contract_addr).unwrap();
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
    const MALAGA_BLOCK_NUMBER: u64 = 2246678;
    const PAIR_ADDRESS_MALAGA: &'static str =
        "wasm15le5evw4regnwf9lrjnpakr2075fcyp4n4yzpelvqcuevzkw2lss46hslz";
    const TOKEN_ADDRESS_MALAGA: &'static str =
        "wasm124v54ngky9wxhx87t252x4xfgujmdsu7uhjdugtkkqt39nld0e6st7e64h";

    #[test]
    fn test_swap_basic_testnet() {
        use serde_json::Value::Null;
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let pair_address = Addr::unchecked(PAIR_ADDRESS_MALAGA);
        let token_address = Addr::unchecked(TOKEN_ADDRESS_MALAGA);
        let my_address = model.eoa.clone();

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
        let resp = model.bank.borrow_mut().query(&bank_query).unwrap();
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
        let resp = model.bank.borrow_mut().query(&bank_query).unwrap();
        let resp_bank: BalanceResponse = serde_json::from_slice(resp.as_slice()).unwrap();
        let umlg_balance_after: u128 = resp_bank.amount.amount.into();

        // token and umlg balance as expected
        assert_eq!(token_balance_after - token_balance_before, 9);
        assert_eq!(umlg_balance_before - umlg_balance_after, 10);
    }

    #[test]
    fn test_cheat_balance() {
        use serde_json::Value::Null;

        let mut model = Model::new(MALAGA_RPC_URL, None).unwrap();
        let my_address = Addr::unchecked(&model.eoa);
        model
            .cheat_bank_balance(&my_address, "umlg", 1_000_000_000)
            .unwrap();
    }
}
