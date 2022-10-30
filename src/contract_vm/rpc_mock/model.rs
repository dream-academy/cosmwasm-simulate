use crate::contract_vm::rpc_mock::{
    Bank, CwRpcClient, RpcContractInstance, RpcMockApi, RpcMockQuerier, RpcMockStorage,
};
use crate::contract_vm::Error;

use cosmwasm_std::{Addr, BankMsg, Coin, ContractInfo, CosmosMsg, Env, Timestamp, WasmMsg};
use cosmwasm_vm::{Backend, InstanceOptions, Storage};
use std::cell::{RefCell, UnsafeCell};
use std::collections::HashMap;
use std::rc::Rc;

type RpcBackend = Backend<RpcMockApi, RpcMockStorage, RpcMockQuerier>;

pub struct Model {
    instances: Rc<UnsafeCell<HashMap<Addr, RpcContractInstance>>>,
    bank: Rc<RefCell<Bank>>,
    client: Rc<RefCell<CwRpcClient>>,
    // similar to tx.origin of solidity
    eoa: String,

    // fields related to blockchain environment
    block_number: u64,
    timestamp: Timestamp,
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
    fn new(rpc_url: &str, block_number: Option<u64>) -> Result<Self, Error> {
        let client = CwRpcClient::new(rpc_url, block_number)?;
        let block_number = client.block_number();
        let timestamp = client.timestamp()?;
        let chain_id = client.chain_id()?;
        let client = Rc::new(RefCell::new(client));
        Ok(Model {
            instances: Rc::new(UnsafeCell::new(HashMap::new())),
            bank: Rc::new(RefCell::new(Bank::new(&client)?)),
            client,
            eoa: BASE_EOA.to_string(),

            block_number,
            timestamp,
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
        let inst = match cosmwasm_vm::Instance::from_code(wasm_code.as_slice(), deps, options, None)
        {
            Err(e) => {
                return Err(Error::vm_error(e));
            }
            Ok(i) => i,
        };
        let instance = RpcContractInstance::make_instance(&contract_addr, inst);
        let instances = unsafe { self.instances.get().as_mut().unwrap() };
        instances.insert(contract_addr.clone(), instance);
        Ok(())
    }

    fn instantiate(&self, code_id: u64) -> Result<(), Error> {
        unimplemented!()
    }

    fn execute(&mut self, contract_addr: &Addr, msg: &[u8], funds: &[Coin]) -> Result<(), Error> {
        let eoa = self.eoa.clone();
        self.execute_inner(contract_addr, &Addr::unchecked(eoa), msg, funds)
    }

    fn execute_inner(
        &mut self,
        contract_addr: &Addr,
        sender: &Addr,
        msg: &[u8],
        funds: &[Coin],
    ) -> Result<(), Error> {
        let env = self.env(contract_addr)?;
        let instances = unsafe { self.instances.get().as_mut().unwrap() };
        if !instances.contains_key(&contract_addr) {
            self.create_instance(contract_addr)?;
        }
        let instance = instances.get_mut(contract_addr).unwrap();
        let response = instance.execute(&env, msg, &sender, funds)?;
        for resp in response.messages {
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
                        self.execute_inner(&target_addr, contract_addr, msg.as_slice(), funds)?;
                    }
                    _ => unimplemented!(),
                },
                CosmosMsg::Bank(bank_msg) => {
                    self.bank.borrow_mut().execute(contract_addr, bank_msg)?;
                }
                _ => unimplemented!(),
            }
        }
        drop(instances);
        self.update_block();
        Ok(())
    }

    /// emulate blockchain block creation
    /// increment block number by 1
    /// increment timestamp by a constant
    fn update_block(&mut self) {
        self.block_number += 1;
        self.timestamp.plus_nanos(BLOCK_EPOCH);
    }

    pub fn new_mock(&self, contract_addr: &Addr) -> Result<RpcBackend, Error> {
        Ok(Backend {
            storage: self.mock_storage(contract_addr)?,
            // is this correct?
            api: RpcMockApi::new(self.canonical_address_length, self.bech32_prefix.as_str())?,
            querier: RpcMockQuerier::new(&self.client, &self.bank, &self.instances),
        })
    }

    pub fn env(&self, contract_addr: &Addr) -> Result<Env, Error> {
        Ok(Env {
            block: cosmwasm_std::BlockInfo {
                height: self.block_number,
                time: self.timestamp.clone(),
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

    pub fn mock_storage(&self, contract_addr: &Addr) -> Result<RpcMockStorage, Error> {
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
}

#[cfg(test)]
mod test {

    use cosmwasm_std::{Addr, Coin, Uint128};
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use crate::contract_vm::rpc_mock::model::Model;

    const MALAGA_RPC_URL: &'static str = "https://rpc.malaga-420.cosmwasm.com:443";
    const MALAGA_BLOCK_NUMBER: u64 = 2246678;
    const PAIR_ADDRESS: &'static str =
        "wasm15le5evw4regnwf9lrjnpakr2075fcyp4n4yzpelvqcuevzkw2lss46hslz";
    const TOKEN_ADDRESS: &'static str =
        "wasm124v54ngky9wxhx87t252x4xfgujmdsu7uhjdugtkkqt39nld0e6st7e64h";

    #[test]
    fn test_swap() {
        use serde_json::Value::Null;
        let mut model = Model::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let prev_block_num = model.block_number;
        let msg_json = json!({
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
        let msg_bytes = serde_json::to_string(&msg_json).unwrap();
        let funds = vec![Coin {
            denom: "umlg".to_string(),
            amount: Uint128::new(10),
        }];
        let pair_address = Addr::unchecked(PAIR_ADDRESS);
        let _ = model
            .execute(&pair_address, msg_bytes.as_bytes(), &funds)
            .unwrap();
        assert_eq!(model.block_number, prev_block_num + 1);
    }
}
