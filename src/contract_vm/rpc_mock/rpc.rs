use std::convert::TryFrom;
use std::future::Future;
use std::str::FromStr;
use tendermint::abci;
use tendermint::block::Height;
use tendermint_rpc::endpoint::abci_query::AbciQuery;
use tendermint_rpc::{Client, HttpClient};
use tokio;

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

    pub fn query_raw(&self, path: &str, data: &str) -> Result<AbciQuery, Error> {
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
}

#[cfg(test)]
mod tests {
    use tonic::transport::Endpoint;

    use crate::contract_vm::rpc_mock::rpc::CwRpcClient;
    use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmos::bank::v1beta1::QueryAllBalancesRequest;
    use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmos::bank::v1beta1::query_client::QueryClient;
    use crate::contract_vm::rpc_mock::rpc::wait_future;
    #[test]
    fn test_rpc_malaga() {
        let rpc_url = "https://rpc.malaga-420.cosmwasm.com:443";
        let client = CwRpcClient::new(rpc_url, None).unwrap();
        let chain_id = client.chain_id().unwrap();
        assert_eq!(chain_id.as_str(), "malaga-420");
        let mut client = wait_future(QueryClient::connect(rpc_url)).unwrap().unwrap();
        let request = tonic::Request::new(QueryAllBalancesRequest {
            address: "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj".to_string(),
            pagination: None,
        });
        let abci_query = wait_future(client.all_balances(request)).unwrap().unwrap();
        println!("{:?}", abci_query);
    }
}
