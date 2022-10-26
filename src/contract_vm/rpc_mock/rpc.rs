use std::convert::TryFrom;
use std::future::Future;
use std::str::FromStr;
use tendermint::abci;
use tendermint::block::Height;
use tendermint_rpc::endpoint::abci_query::AbciQuery;
use tendermint_rpc::{Client, HttpClient};
use tokio;
use prost::Message;

use crate::contract_vm::error::Error;

pub mod rpc_items {
    pub mod cosmos {
        pub mod base {
            pub mod v1beta1 {
                tonic::include_proto!("cosmos.base.v1beta1");
            }
            pub mod query {
                pub mod v1beta1 {
                    tonic::include_proto!("cosmos.base.query.v1beta1");
                }
            }
        }
        pub mod bank {
            pub mod v1beta1 {
                tonic::include_proto!("cosmos.bank.v1beta1");
            }
        }
    }
}

pub struct CwRpcClient {
    _inner: HttpClient,
    block_number: u64,
}

// protobuf serialize + hexencode
fn serialize<M: Message>(m: &M) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    match m.encode(&mut out) {
        Ok(_) => {
            Ok(out)
        },
        Err(e) => Err(Error::protobuf_error(e))
    }
}

fn wait_future<F: Future>(f: F) -> Result<F::Output, Error> {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(b) => Ok(b.block_on(f)),
        Err(e) => Err(Error::tokio_error(e)),
    }
}

impl CwRpcClient {
    pub fn new(url: &str, block_number: Option<u64>) -> Result<Self, Error> {
        let mut rv = Self {
            _inner: match HttpClient::new(url) {
                Ok(h) => h,
                Err(e) => {
                    return Err(Error::rpc_error(e));
                }
            },
            block_number: 0,
        };
        let block_height = rv.block_height()?;
        if let Some(bn) = block_number {
            if bn > block_height {
                let msg = format!("invalid block number, exceeds height({})", block_height);
                Err(Error::invalid_argument(msg))
            } else {
                rv.block_number = bn;
                Ok(rv)
            }
        } else {
            rv.block_number = block_height;
            Ok(rv)
        }
    }

    pub fn chain_id(&self) -> Result<String, Error> {
        let status = match wait_future(self._inner.status()) {
            Ok(r) => match r {
                Ok(r) => r,
                Err(e) => {
                    return Err(Error::rpc_error(e));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };
        Ok(status.node_info.network.to_string())
    }

    pub fn block_height(&self) -> Result<u64, Error> {
        let status = match wait_future(self._inner.status()) {
            Ok(r) => match r {
                Ok(r) => r,
                Err(e) => {
                    return Err(Error::rpc_error(e));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };
        Ok(status.sync_info.latest_block_height.value())
    }

    pub fn query_raw(&self, path: &str, data: &[u8]) -> Result<AbciQuery, Error> {
        let path = match abci::Path::from_str(path) {
            Ok(p) => p,
            Err(e) => {
                return Err(Error::tendermint_error(e));
            }
        };
        let height = match Height::try_from(self.block_number) {
            Ok(h) => h,
            Err(e) => {
                return Err(Error::tendermint_error(e));
            }
        };
        let result =
            match wait_future(
                self._inner
                    .abci_query(Some(path), data, Some(height), false),
            ) {
                Ok(r) => match r {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(Error::tendermint_error(e));
                    }
                },
                Err(e) => {
                    return Err(e);
                }
            };
        Ok(result)
    }

    pub fn query_bank_balance_all(&self, address: &str) -> Result<Vec<(String, u64)>, Error> {
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmos::bank::v1beta1::QueryAllBalancesRequest;
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmos::bank::v1beta1::QueryAllBalancesResponse;
        let request = QueryAllBalancesRequest {
            address: address.to_string(),
            pagination: None,
        };
        let path = "/cosmos.bank.v1beta1.Query/AllBalances";
        let data = serialize(&request).unwrap();
        let out = self.query_raw(path, data.as_slice()).unwrap();
        match out.code {
            abci::Code::Ok => {
                let resp = match QueryAllBalancesResponse::decode(out.value.as_slice()) {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(Error::protobuf_error(e));
                    }
                };
                let balances: Vec<(String, u64)> = resp.balances.iter().map(|x| {
                    (x.denom.to_string(), u64::from_str(&x.amount).unwrap())
                }).collect();
                Ok(balances)
            },
            _ => {
                Err(Error::tendermint_error(out.log))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tonic::transport::Endpoint;

    use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmos::bank::v1beta1::query_client::QueryClient;
    use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmos::bank::v1beta1::QueryAllBalancesRequest;
    use crate::contract_vm::rpc_mock::rpc::wait_future;
    use crate::contract_vm::rpc_mock::rpc::serialize;
    use crate::contract_vm::rpc_mock::rpc::CwRpcClient;
    #[test]
    fn test_rpc_malaga() {
        let rpc_url = "https://rpc.malaga-420.cosmwasm.com:443";
        let client = CwRpcClient::new(rpc_url, None).unwrap();
        let chain_id = client.chain_id().unwrap();
        assert_eq!(chain_id.as_str(), "malaga-420");
        let address = "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj";
        let balances = client.query_bank_balance_all(address).unwrap();
        assert_eq!(balances[0].0.as_str(), "umlg");
    }
}
