use crate::contract_vm::rpc_mock::CwRpcClient;
use crate::contract_vm::Error;
use cosmwasm_std::{
    to_binary, Addr, AllBalanceResponse, BalanceResponse, BankMsg, BankQuery, Binary, Coin,
    ContractResult, Event, Response, Uint128,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Bank {
    // address -> ( denom -> amount )
    balances: HashMap<Addr, HashMap<String, Uint128>>,
    client: Arc<Mutex<CwRpcClient>>,
}
