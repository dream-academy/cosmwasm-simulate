extern crate cosmwasm_std;
extern crate cosmwasm_vm;
extern crate serde_json;
extern crate wasmer;
use std::fmt::Write;
use wasmer::{Module, Store, Universal};
use wasmer_compiler_singlepass::Singlepass;

use self::cosmwasm_std::{Addr, Binary, ContractInfo, ContractResult, Response, Timestamp};
use self::cosmwasm_vm::testing::MockQuerier;
use self::cosmwasm_vm::{Instance, InstanceOptions};
use crate::contract_vm::{analyzer, mock};

static DEFAULT_GAS_LIMIT: u64 = 500_000;

pub struct ContractInstance {
    pub module: Module,
    pub instance: Instance<mock::MockApi, mock::MockStorage, MockQuerier>,
    pub wasm_file: String,
    pub env: cosmwasm_std::Env,
    pub analyzer: analyzer::Analyzer,
}

impl ContractInstance {
    pub fn new_instance(wasm_file: &str) -> Result<Self, String> {
        let deps = mock::new_mock(20, &[], "fake_contract_addr");
        let wasm = match analyzer::load_data_from_file(wasm_file) {
            Err(e) => return Err(e),
            Ok(code) => code,
        };
        println!("Compiling code");
        let compiler = Singlepass::default();
        let engine = Universal::new(compiler).engine();
        let store = Store::new(&engine);
        let module = match Module::new(&store, wasm.as_slice()) {
            Ok(m) => m,
            Err(e) => {
                return Err(format!("{:?}", e));
            }
        };
        let default_options = InstanceOptions {
            gas_limit: DEFAULT_GAS_LIMIT,
            print_debug: false,
        };
        let inst =
            match cosmwasm_vm::Instance::from_code(wasm.as_slice(), deps, default_options, None) {
                Err(e) => {
                    println!("cosmwasm_vm::Instance::from_code return error {}", e);
                    return Err("Instance from code execute failed!".to_string());
                }
                Ok(i) => i,
            };
        return Ok(ContractInstance::make_instance(
            module,
            inst,
            wasm_file.to_string(),
        ));
    }

    fn make_instance(
        md: Module,
        inst: cosmwasm_vm::Instance<mock::MockApi, mock::MockStorage, MockQuerier>,
        file: String,
    ) -> ContractInstance {
        return ContractInstance {
            module: md,
            instance: inst,
            wasm_file: file,
            env: ContractInstance::build_mock_env(),
            analyzer: analyzer::Analyzer::default(),
        };
    }

    fn build_mock_env() -> cosmwasm_std::Env {
        return cosmwasm_std::Env {
            block: cosmwasm_std::BlockInfo {
                height: 0,
                time: Timestamp::from_seconds(0),
                chain_id: "okchain".to_string(),
            },
            transaction: Some(cosmwasm_std::TransactionInfo { index: 0 }),
            contract: ContractInfo {
                address: Addr::unchecked("abcd"),
            },
        };
    }

    pub fn show_module_info(&self) {
        println!("showing wasm module info for [{}]", self.wasm_file);

        println!("=============================== module info exported func name ===============================");
        for exdesc in self.module.exports() {
            println!("exported func name [{}]", exdesc.name());
        }
        println!("=============================== module info exported func name ===============================");
        for desc in self.module.imports() {
            println!(
                "import descriptor name:[{}->{}]",
                desc.module(),
                desc.name()
            );
        }
    }

    fn dump_result(key: &str, value: &[u8]) {
        let mut value_str = match std::str::from_utf8(value) {
            Ok(result) => result.to_string(),
            _ => "".to_string(),
        };

        if value_str.is_empty() {
            for a in value.iter() {
                write!(value_str, "{:02x}", a).expect("Not written");
            }
        }

        println!("{} = {}", key, value_str);
    }
    pub fn call(&mut self, func_type: String, param: String) -> String {
        println!("***************************call started***************************");
        println!("executing func [{}] , params is {}", func_type, param);
        let gas_init = self.instance.get_gas_left();
        if func_type == "init" {
            let info = cosmwasm_std::MessageInfo {
                sender: Addr::unchecked("abcd"),
                funds: vec![],
            };
            let init_result = cosmwasm_vm::call_instantiate(
                &mut self.instance,
                &self.env,
                &info,
                param.as_bytes(),
            );
            let msg: Response<cosmwasm_std::Empty> = match init_result {
                Ok(data) => match data {
                    ContractResult::Ok(resp) => resp,
                    ContractResult::Err(err) => {
                        println!("Error {}", err);
                        return "ERROR      :execute init failed".to_string();
                    }
                },
                Err(err) => {
                    println!("Error {}", err);
                    return "ERROR      :execute init failed".to_string();
                }
            };
            let data: Binary = match msg.data {
                None => Binary::from("".as_bytes()),
                Some(d) => d,
            };
            ContractInstance::dump_result("init msg.data:", data.0.as_slice());
        } else if func_type == "execute" {
            let info = cosmwasm_std::MessageInfo {
                sender: cosmwasm_std::Addr::unchecked("abcd"),
                funds: vec![],
            };
            let handle_result =
                cosmwasm_vm::call_execute(&mut self.instance, &self.env, &info, param.as_bytes());
            let msg: Response<cosmwasm_std::Empty> = match handle_result {
                Ok(data) => match data {
                    ContractResult::Ok(resp) => resp,
                    ContractResult::Err(err) => {
                        println!("Error {}", err);
                        return "ERROR      :execute query failed".to_string();
                    }
                },
                Err(err) => {
                    println!("Error {}", err);
                    return "ERROR      :execute query failed".to_string();
                }
            };

            let data: Binary = match msg.data {
                None => Binary::from("".as_bytes()),
                Some(d) => d,
            };
            ContractInstance::dump_result("handle msg.data:", data.0.as_slice());
        } else if func_type == "query" {
            let query_result =
                cosmwasm_vm::call_query(&mut self.instance, &self.env, param.as_bytes());
            let msg = match query_result {
                Ok(data) => match data {
                    ContractResult::Ok(resp) => resp,
                    ContractResult::Err(err) => {
                        println!("Error {}", err);
                        return "ERROR      :execute query failed".to_string();
                    }
                },
                Err(err) => {
                    println!("Error {}", err);
                    return "ERROR      :execute query failed".to_string();
                }
            };

            ContractInstance::dump_result("query msg.data:", msg.0.as_slice());
        } else {
            println!("wrong dispatcher call {}", func_type);
        }
        let gas_used = gas_init - self.instance.get_gas_left();
        println!("Gas used   : {}", gas_used);
        println!("***************************call finished***************************");
        return "Execute Success".to_string();
    }
}
