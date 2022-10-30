use crate::contract_vm::rpc_mock::{Bank, CwRpcClient};
use cosmwasm_std::from_slice;
use cosmwasm_std::{Binary, ContractResult, QueryRequest, SystemResult};
use cosmwasm_vm::{BackendError, BackendResult, GasInfo, Querier};
use lazy_static::__Deref;

use std::cell::RefCell;
use std::rc::Rc;

pub struct RpcMockQuerier {
    client: Rc<RefCell<CwRpcClient>>,
    bank: Rc<RefCell<Bank>>,
}

impl Querier for RpcMockQuerier {
    fn query_raw(
        &self,
        request: &[u8],
        gas_limit: u64,
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
                let mut bank = self.bank.borrow_mut();
                bank.query(bank_query, &mut self.client.borrow_mut());
                panic!("a")
            }
            QueryRequest::Wasm(wasm_query) => {
                panic!("b")
            }
            _ => unimplemented!(),
        }
    }
}

impl RpcMockQuerier {
    pub fn new(client: &Rc<RefCell<CwRpcClient>>, bank: &Rc<RefCell<Bank>>) -> Self {
        Self {
            client: Rc::clone(client),
            bank: Rc::clone(bank),
        }
    }
}
