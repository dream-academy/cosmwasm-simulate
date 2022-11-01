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

impl Bank {
    pub fn new(client: &Arc<Mutex<CwRpcClient>>) -> Result<Self, Error> {
        Ok(Bank {
            balances: HashMap::new(),
            client: Arc::clone(client),
        })
    }

    pub fn get_balance(&mut self, owner: &Addr, denom: &str) -> Result<Uint128, Error> {
        if !self.balances.contains_key(owner) {
            let mut client = self.client.lock().unwrap();
            self.balances.insert(
                owner.clone(),
                client
                    .query_bank_all_balances(owner.as_str())?
                    .iter()
                    .map(|(d, a)| (d.clone(), Uint128::new(*a)))
                    .collect::<HashMap<String, Uint128>>(),
            );
        }
        let balances = self.balances.get_mut(owner).unwrap();
        if let Some(balance) = balances.get(denom) {
            Ok(balance.clone())
        } else {
            Ok(Uint128::new(0))
        }
    }

    pub fn get_balances(&mut self, owner: &Addr) -> Result<Vec<Coin>, Error> {
        if !self.balances.contains_key(owner) {
            let mut client = self.client.lock().unwrap();
            self.balances.insert(
                owner.clone(),
                client
                    .query_bank_all_balances(owner.as_str())?
                    .iter()
                    .map(|(d, a)| (d.clone(), Uint128::new(*a)))
                    .collect::<HashMap<String, Uint128>>(),
            );
        }
        Ok(self
            .balances
            .get_mut(owner)
            .unwrap()
            .iter()
            .map(|(k, v)| Coin {
                denom: k.clone(),
                amount: v.clone(),
            })
            .collect())
    }

    pub fn set_balance(
        &mut self,
        owner: &Addr,
        denom: &str,
        balance: Uint128,
    ) -> Result<(), Error> {
        self.balances
            .entry(owner.clone())
            .or_insert_with(HashMap::new)
            .insert(denom.to_string(), balance);
        Ok(())
    }

    fn send_internal(
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
                events.push(coin_spent_event(src, coin.amount, &coin.denom));
                events.push(coin_received_event(dst, coin.amount, &coin.denom));
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

    fn burn_internal(
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

    pub fn execute(
        &mut self,
        sender: &Addr,
        bank_msg: &BankMsg,
    ) -> Result<ContractResult<Response>, Error> {
        match bank_msg {
            BankMsg::Send { to_address, amount } => {
                let dst = Addr::unchecked(to_address);
                self.send_internal(sender, &dst, amount)
            }
            BankMsg::Burn { amount } => self.burn_internal(sender, amount),
            _ => unimplemented!(),
        }
    }

    /// queries the bank structure maintained in-memory
    /// if the in-memory db is not capable of handling the query, use the RPC client
    pub fn query(&mut self, bank_query: &BankQuery) -> Result<Binary, Error> {
        match bank_query {
            BankQuery::Balance { address, denom } => {
                let balance = self.get_balance(&Addr::unchecked(address), &denom)?;
                let response = BalanceResponse {
                    amount: Coin {
                        denom: denom.to_string(),
                        amount: balance,
                    },
                };
                Ok(to_binary(&response).map_err(|e| Error::std_error(e))?)
            }
            BankQuery::AllBalances { address } => {
                let balances = self.get_balances(&Addr::unchecked(address))?;
                let response = AllBalanceResponse { amount: balances };
                Ok(to_binary(&response).map_err(|e| Error::std_error(e))?)
            }
            _ => unimplemented!(),
        }
    }
}
