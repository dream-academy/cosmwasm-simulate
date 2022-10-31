use crate::contract_vm::rpc_mock::CwRpcClient;
use crate::contract_vm::Error;
use cosmwasm_std::{
    to_binary, Addr, AllBalanceResponse, BalanceResponse, BankMsg, BankQuery, Binary, Coin, Uint128,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct Bank {
    // address -> ( denom -> amount )
    balances: HashMap<Addr, HashMap<String, Uint128>>,
    client: Rc<RefCell<CwRpcClient>>,
}

impl Bank {
    pub fn new(client: &Rc<RefCell<CwRpcClient>>) -> Result<Self, Error> {
        Ok(Bank {
            balances: HashMap::new(),
            client: Rc::clone(client),
        })
    }

    pub fn get_balance(&mut self, owner: &Addr, denom: &str) -> Result<Uint128, Error> {
        if !self.balances.contains_key(owner) {
            let mut client = self.client.borrow_mut();
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
            let mut client = self.client.borrow_mut();
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

    fn send_internal(&mut self, src: &Addr, dst: &Addr, amount: &[Coin]) -> Result<(), Error> {
        for coin in amount.iter() {
            let src_amount = self.get_balance(src, &coin.denom)?;
            let dst_amount = self.get_balance(dst, &coin.denom)?;
            if src_amount >= coin.amount {
                self.set_balance(src, &coin.denom, src_amount - coin.amount)?;
                self.set_balance(dst, &coin.denom, dst_amount + coin.amount)?;
            } else {
                return Err(Error::bank_error(&format!(
                    "insufficient balance (owner: {}, balance: {}, amount: {})",
                    src, src_amount, coin.amount
                )));
            }
        }
        Ok(())
    }

    fn burn_internal(&mut self, src: &Addr, amount: &[Coin]) -> Result<(), Error> {
        for coin in amount.iter() {
            let src_amount = self.get_balance(src, &coin.denom)?;
            if src_amount >= coin.amount {
                self.set_balance(src, &coin.denom, src_amount - coin.amount)?;
            } else {
                return Err(Error::bank_error(&format!(
                    "insufficient balance (owner: {}, balance: {}, amount: {})",
                    src, src_amount, coin.amount
                )));
            }
        }
        Ok(())
    }

    pub fn execute(&mut self, sender: &Addr, bank_msg: &BankMsg) -> Result<(), Error> {
        match bank_msg {
            BankMsg::Send { to_address, amount } => {
                let dst = Addr::unchecked(to_address);
                self.send_internal(sender, &dst, amount)?;
            }
            BankMsg::Burn { amount } => {
                self.burn_internal(sender, amount)?;
            }
            _ => unimplemented!(),
        }
        Ok(())
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
