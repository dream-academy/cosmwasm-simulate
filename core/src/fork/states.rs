use crate::CwClientBackend;
use crate::Error;
use cosmwasm_std::{
    to_binary, Addr, AllBalanceResponse, BalanceResponse, BankMsg, BankQuery, Binary, Coin,
    ContractResult, Event, Response, Timestamp, Uint128,
};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

pub type ContractStorage = BTreeMap<Vec<u8>, Vec<u8>>;

const BLOCK_EPOCH: u64 = 1_000_000_000;

/// techically contract code is not part of contract state, but we just name it as 'state' for simplicity
pub struct ContractState {
    pub code: Vec<u8>,
    pub storage: Arc<RwLock<ContractStorage>>,
}

impl Clone for ContractState {
    fn clone(&self) -> Self {
        Self {
            code: self.code.clone(),
            storage: Arc::new(RwLock::new(self.storage.read().unwrap().clone())),
        }
    }
}

#[derive(Clone)]
pub struct AllStates {
    contract_states: HashMap<Addr, ContractState>,
    bank_states: HashMap<Addr, HashMap<String, Uint128>>,
    pub client: Box<dyn CwClientBackend>,
    // fields related to blockchain environment
    pub block_number: u64,
    pub block_timestamp: Timestamp,
    pub chain_id: String,
    pub canonical_address_length: usize,
    pub bech32_prefix: String,
}

impl AllStates {
    pub fn new(
        client: Box<dyn CwClientBackend>,
        canonical_address_length: usize,
        bech32_prefix: &str,
    ) -> Result<Self, Error> {
        let mut client = client;
        let block_number = client.block_number();
        let block_timestamp = client.timestamp()?;
        let chain_id = client.chain_id()?;
        Ok(Self {
            contract_states: HashMap::new(),
            bank_states: HashMap::new(),
            client,
            block_number,
            block_timestamp,
            chain_id,
            canonical_address_length,
            bech32_prefix: bech32_prefix.to_string(),
        })
    }

    pub fn contract_state_insert(&mut self, contract_addr: Addr, contract_state: ContractState) {
        self.contract_states.insert(contract_addr, contract_state);
    }

    pub fn contract_state_remove(&mut self, contract_addr: &Addr) {
        self.contract_states.remove(contract_addr);
    }

    pub fn contract_storage_update(&mut self, contract_addr: &Addr, new_storage: ContractStorage) {
        *self
            .contract_states
            .get_mut(contract_addr)
            .unwrap()
            .storage
            .write()
            .unwrap() = new_storage;
    }

    pub fn contract_state_get(&self, contract_addr: &Addr) -> Option<&ContractState> {
        self.contract_states.get(contract_addr)
    }

    pub fn contract_state_get_mut(&mut self, contract_addr: &Addr) -> Option<&mut ContractState> {
        self.contract_states.get_mut(contract_addr)
    }

    pub fn insert_bank_state(&mut self, addr: Addr, balances: HashMap<String, Uint128>) {
        self.bank_states.insert(addr, balances);
    }

    pub fn get_bank_state(&self, addr: &Addr) -> Option<&HashMap<String, Uint128>> {
        self.bank_states.get(addr)
    }

    pub fn bank_state_entry(&mut self, addr: Addr) -> Entry<Addr, HashMap<String, Uint128>> {
        self.bank_states.entry(addr)
    }

    /// emulate blockchain block creation
    /// increment block number by 1
    /// increment timestamp by a constant
    pub fn update_block(&mut self) {
        self.block_number += 1;
        self.block_timestamp.plus_nanos(BLOCK_EPOCH);
    }

    fn coin_spent_event(sender: &Addr, amount: Uint128, denom: &str) -> Event {
        Event::new("coin_spent")
            .add_attribute("spender", sender)
            .add_attribute("amount", format!("{}{}", amount, denom))
    }

    fn coin_received_event(receiver: &Addr, amount: Uint128, denom: &str) -> Event {
        Event::new("coin_received")
            .add_attribute("receiver", receiver)
            .add_attribute("amount", format!("{}{}", amount, denom))
    }

    pub fn get_balance(&mut self, owner: &Addr, denom: &str) -> Result<Uint128, Error> {
        if self.get_bank_state(owner).is_none() {
            let balances: HashMap<String, Uint128> = self
                .client
                .query_bank_all_balances(owner.as_str())?
                .iter()
                .map(|(d, a)| (d.clone(), Uint128::new(*a)))
                .collect();
            self.insert_bank_state(owner.clone(), balances);
        }

        let balances = self.get_bank_state(owner).unwrap();
        if let Some(balance) = balances.get(denom) {
            Ok(*balance)
        } else {
            Ok(Uint128::new(0))
        }
    }

    pub fn get_balances(&mut self, owner: &Addr) -> Result<Vec<Coin>, Error> {
        if self.get_bank_state(owner).is_none() {
            let balances: HashMap<String, Uint128> = self
                .client
                .query_bank_all_balances(owner.as_str())?
                .iter()
                .map(|(d, a)| (d.clone(), Uint128::new(*a)))
                .collect();
            self.insert_bank_state(owner.clone(), balances);
        }

        let balances = self.get_bank_state(owner).unwrap();
        let coins: Vec<Coin> = balances
            .iter()
            .map(|(d, v)| Coin {
                denom: d.to_string(),
                amount: *v,
            })
            .collect();
        Ok(coins)
    }

    pub fn set_balance(
        &mut self,
        owner: &Addr,
        denom: &str,
        balance: Uint128,
    ) -> Result<(), Error> {
        self.bank_state_entry(owner.clone())
            .or_insert_with(HashMap::new)
            .insert(denom.to_string(), balance);
        Ok(())
    }

    fn bank_send(
        &mut self,
        src: &Addr,
        dst: &Addr,
        amount: &[Coin],
    ) -> Result<ContractResult<Response>, Error> {
        let mut events = Vec::new();
        for coin in amount.iter() {
            let src_amount = self.get_balance(src, &coin.denom)?;
            let dst_amount = self.get_balance(dst, &coin.denom)?;
            if src_amount >= coin.amount {
                self.set_balance(src, &coin.denom, src_amount - coin.amount)?;
                self.set_balance(dst, &coin.denom, dst_amount + coin.amount)?;
                events.push(Self::coin_spent_event(src, coin.amount, &coin.denom));
                events.push(Self::coin_received_event(dst, coin.amount, &coin.denom));
            } else {
                return Ok(ContractResult::Err(format!(
                    "insufficient balance (owner: {}, balance: {}, amount: {})",
                    src, src_amount, coin.amount
                )));
            }
        }
        // TODO: make this more verbose
        let response = Response::new().add_events(events);
        Ok(ContractResult::Ok(response))
    }

    fn bank_burn(
        &mut self,
        src: &Addr,
        amount: &[Coin],
    ) -> Result<ContractResult<Response>, Error> {
        for coin in amount.iter() {
            let src_amount = self.get_balance(src, &coin.denom)?;
            if src_amount >= coin.amount {
                self.set_balance(src, &coin.denom, src_amount - coin.amount)?;
            } else {
                return Ok(ContractResult::Err(format!(
                    "insufficient balance (owner: {}, balance: {}, amount: {})",
                    src, src_amount, coin.amount
                )));
            }
        }
        // TODO: make this more verbose
        let response = Response::new();
        Ok(ContractResult::Ok(response))
    }

    pub fn bank_execute(
        &mut self,
        sender: &Addr,
        bank_msg: &BankMsg,
    ) -> Result<ContractResult<Response>, Error> {
        match bank_msg {
            BankMsg::Send { to_address, amount } => {
                let dst = Addr::unchecked(to_address);
                self.bank_send(sender, &dst, amount)
            }
            BankMsg::Burn { amount } => self.bank_burn(sender, amount),
            _ => unimplemented!(),
        }
    }

    /// queries the bank structure maintained in-memory
    /// if the in-memory db is not capable of handling the query, use the RPC client
    pub fn bank_query(&mut self, bank_query: &BankQuery) -> Result<Binary, Error> {
        match bank_query {
            BankQuery::Balance { address, denom } => {
                let balance = self.get_balance(&Addr::unchecked(address), denom)?;
                let response = BalanceResponse {
                    amount: Coin {
                        denom: denom.to_string(),
                        amount: balance,
                    },
                };
                Ok(to_binary(&response).map_err(Error::std_error)?)
            }
            BankQuery::AllBalances { address } => {
                let balances = self.get_balances(&Addr::unchecked(address))?;
                let response = AllBalanceResponse { amount: balances };
                Ok(to_binary(&response).map_err(Error::std_error)?)
            }
            _ => unimplemented!(),
        }
    }
}
