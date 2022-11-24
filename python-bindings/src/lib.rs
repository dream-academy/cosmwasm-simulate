use std::collections::HashMap;

use cosmwasm_simulate::{Addr, Coin, Timestamp, Uint128};
// we don't import Model and DebugLog in order to use their names for Python classes
use pyo3::{exceptions::PyRuntimeError, prelude::*};

#[pyclass]
struct Model {
    inner: cosmwasm_simulate::Model,
}

#[pyclass]
struct DebugLog {
    inner: cosmwasm_simulate::DebugLog,
}

#[pymethods]
impl DebugLog {
    fn get_log(self_: PyRefMut<Self>) -> PyResult<Vec<String>> {
        let debug_log = &self_.inner;
        let mut out = Vec::new();
        for d in debug_log.logs.iter() {
            out.push(format!("{}", d));
        }
        Ok(out)
    }

    fn get_err_msg(self_: PyRefMut<Self>) -> PyResult<String> {
        let debug_log = &self_.inner;
        if let Some(err_msg) = &debug_log.err_msg {
            Ok(err_msg.to_string())
        } else {
            Ok("".to_string())
        }
    }

    fn get_stdout(self_: PyRefMut<Self>) -> PyResult<String> {
        let debug_log = &self_.inner;
        Ok(debug_log.get_stdout())
    }
}

#[pymethods]
impl Model {
    #[new]
    fn new(url: String, block_number: Option<u64>, bech32_prefix: String) -> PyResult<Model> {
        let model = cosmwasm_simulate::Model::new(&url, block_number, &bech32_prefix)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Model { inner: model })
    }

    pub fn block_number(mut self_: PyRefMut<Self>) -> PyResult<u64> {
        let model = &mut self_.inner;
        Ok(model.block_number())
    }

    pub fn add_custom_code(mut self_: PyRefMut<Self>, code_id: u64, code: &[u8]) -> PyResult<()> {
        let model = &mut self_.inner;
        model
            .add_custom_code(code_id, code)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    pub fn instantiate(
        mut self_: PyRefMut<Self>,
        code_id: u64,
        msg: &[u8],
        funds_: Vec<(String, u128)>,
    ) -> PyResult<DebugLog> {
        let model = &mut self_.inner;
        let funds: Vec<Coin> = funds_
            .iter()
            .map(|(d, a)| Coin {
                denom: d.to_string(),
                amount: Uint128::new(*a),
            })
            .collect();
        let debug_log = model
            .instantiate(code_id, msg, &funds)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(DebugLog { inner: debug_log })
    }

    pub fn execute(
        mut self_: PyRefMut<Self>,
        contract_addr_: &str,
        msg: &[u8],
        funds_: Vec<(String, u128)>,
    ) -> PyResult<DebugLog> {
        let model = &mut self_.inner;
        let funds: Vec<Coin> = funds_
            .iter()
            .map(|(d, a)| Coin {
                denom: d.to_string(),
                amount: Uint128::new(*a),
            })
            .collect();
        let contract_addr = Addr::unchecked(contract_addr_);
        let debug_log = model
            .execute(&contract_addr, msg, &funds)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(DebugLog { inner: debug_log })
    }

    pub fn wasm_query(
        mut self_: PyRefMut<Self>,
        contract_addr_: &str,
        msg: &[u8],
    ) -> PyResult<Vec<u8>> {
        let model = &mut self_.inner;
        let contract_addr = Addr::unchecked(contract_addr_);
        let out = model
            .wasm_query(&contract_addr, msg)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(out.to_vec())
    }

    pub fn bank_query(mut self_: PyRefMut<Self>, msg: &[u8]) -> PyResult<Vec<u8>> {
        let model = &mut self_.inner;
        let out = model
            .bank_query(msg)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(out.to_vec())
    }

    pub fn cheat_block_number(mut self_: PyRefMut<Self>, block_number: u64) -> PyResult<()> {
        let model = &mut self_.inner;
        model
            .cheat_block_number(block_number)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    /// set latest block timestamp, units in nanoseconds
    pub fn cheat_block_timestamp(mut self_: PyRefMut<Self>, timestamp_: u64) -> PyResult<()> {
        let model = &mut self_.inner;
        let timestamp = Timestamp::from_nanos(timestamp_);
        model
            .cheat_block_timestamp(timestamp)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    pub fn cheat_bank_balance(
        mut self_: PyRefMut<Self>,
        addr_: &str,
        amount: (String, u128),
    ) -> PyResult<()> {
        let model = &mut self_.inner;
        let addr = Addr::unchecked(addr_);
        let (denom, new_balance) = amount;
        model
            .cheat_bank_balance(&addr, &denom, new_balance)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    pub fn cheat_code(
        mut self_: PyRefMut<Self>,
        contract_addr_: &str,
        code: &[u8],
    ) -> PyResult<()> {
        let model = &mut self_.inner;
        let contract_addr = Addr::unchecked(contract_addr_);
        model
            .cheat_code(&contract_addr, code)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    pub fn cheat_message_sender(mut self_: PyRefMut<Self>, sender: &str) -> PyResult<()> {
        let model = &mut self_.inner;
        let sender_addr = Addr::unchecked(sender);
        model
            .cheat_message_sender(&sender_addr)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    pub fn cheat_storage(
        mut self_: PyRefMut<Self>,
        contract_addr: &str,
        key: &[u8],
        value: &[u8],
    ) -> PyResult<()> {
        let model = &mut self_.inner;
        let contract_addr = Addr::unchecked(contract_addr);
        model
            .cheat_storage(&contract_addr, key, value)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    pub fn enable_code_coverage(mut self_: PyRefMut<Self>) -> PyResult<()> {
        let model = &mut self_.inner;
        model.enable_code_coverage();
        Ok(())
    }

    pub fn disable_code_coverage(mut self_: PyRefMut<Self>) -> PyResult<()> {
        let model = &mut self_.inner;
        model.disable_code_coverage();
        Ok(())
    }

    pub fn get_code_coverage(mut self_: PyRefMut<Self>) -> PyResult<HashMap<String, Vec<Vec<u8>>>> {
        let model = &mut self_.inner;
        Ok(model.get_coverage())
    }
}

/// CosmWasm Simulator framework with Python bindings
#[pymodule]
fn cwsimpy(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Model>()?;
    m.add_class::<DebugLog>()?;
    Ok(())
}
