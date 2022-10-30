use crate::contract_vm::rpc_mock::CwRpcClient;
use crate::contract_vm::Error;
use cosmwasm_std::{
    to_binary, AllBalanceResponse, BalanceResponse, BankQuery, Binary, Coin, Uint128,
};
use std::collections::HashMap;

pub struct Bank {
    // address -> ( denom -> amount )
    balances: HashMap<String, HashMap<String, Uint128>>,
}

impl Bank {
    pub fn new() -> Result<Self, Error> {
        Ok(Bank {
            balances: HashMap::new(),
        })
    }

    /// queries the bank structure maintained in-memory
    /// if the in-memory db is not capable of handling the query, use the RPC client
    pub fn query(
        &mut self,
        bank_query: BankQuery,
        rpc_client: &mut CwRpcClient,
    ) -> Result<Binary, Error> {
        match bank_query {
            BankQuery::Balance { address, denom } => {
                let balance: u128 = if let Some(coins) = self.balances.get(&address) {
                    if let Some(amount) = coins.get(&denom) {
                        amount.u128()
                    } else {
                        0
                    }
                } else {
                    let balance = rpc_client.query_bank_all_balances(&address)?;
                    let mut amount = None;
                    for (d, a) in balance {
                        if denom == d {
                            amount = Some(a);
                            break;
                        }
                    }
                    if let Some(amount) = amount {
                        amount
                    } else {
                        0
                    }
                };
                let response = BalanceResponse {
                    amount: Coin {
                        denom,
                        amount: Uint128::new(balance),
                    },
                };
                Ok(to_binary(&response).map_err(|e| Error::std_error(e))?)
            }
            BankQuery::AllBalances { address } => {
                let balances: Vec<Coin> = if let Some(coins) = self.balances.get(&address) {
                    coins
                        .iter()
                        .map(|(d, a)| Coin {
                            denom: d.clone(),
                            amount: a.clone(),
                        })
                        .collect()
                } else {
                    rpc_client
                        .query_bank_all_balances(&address)?
                        .iter()
                        .map(|(d, a)| Coin {
                            denom: d.clone(),
                            amount: Uint128::new(*a),
                        })
                        .collect()
                };
                let response = AllBalanceResponse { amount: balances };
                Ok(to_binary(&response).map_err(|e| Error::std_error(e))?)
            }
            _ => unimplemented!(),
        }
    }
}
