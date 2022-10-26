use prost::Message;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::future::Future;
use std::str::FromStr;
use tendermint::abci;
use tendermint::block::Height;
use tendermint_rpc::endpoint::abci_query::AbciQuery;
use tendermint_rpc::{Client, HttpClient};
use tokio;

use crate::contract_vm::error::Error;

macro_rules! include_proto {
    ($x: literal) => {
        include!(concat!(env!("OUT_DIR"), "/", $x, ".rs"));
    };
}

pub mod rpc_items {
    pub mod cosmwasm {
        pub mod wasm {
            pub mod v1 {
                include_proto!("cosmwasm.wasm.v1");
            }
        }
    }
    pub mod cosmos {
        pub mod base {
            pub mod v1beta1 {
                include_proto!("cosmos.base.v1beta1");
            }
            pub mod query {
                pub mod v1beta1 {
                    include_proto!("cosmos.base.query.v1beta1");
                }
            }
        }
        pub mod bank {
            pub mod v1beta1 {
                include_proto!("cosmos.bank.v1beta1");
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
        Ok(_) => Ok(out),
        Err(e) => Err(Error::protobuf_error(e)),
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

    pub fn query_bank_all_balances(&self, address: &str) -> Result<Vec<(String, u64)>, Error> {
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
                let balances: Vec<(String, u64)> = resp
                    .balances
                    .iter()
                    .map(|x| (x.denom.to_string(), u64::from_str(&x.amount).unwrap()))
                    .collect();
                Ok(balances)
            }
            _ => Err(Error::tendermint_error(out.log)),
        }
    }

    pub fn query_wasm_contract_smart(
        &self,
        address: &str,
        query_data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmwasm::wasm::v1::QuerySmartContractStateRequest;
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmwasm::wasm::v1::QuerySmartContractStateResponse;
        let request = QuerySmartContractStateRequest {
            address: address.to_string(),
            query_data: query_data.to_vec(),
        };
        let path = "/cosmwasm.wasm.v1.Query/SmartContractState";
        let data = serialize(&request).unwrap();
        let out = self.query_raw(path, data.as_slice()).unwrap();
        match out.code {
            abci::Code::Ok => {
                let resp = match QuerySmartContractStateResponse::decode(out.value.as_slice()) {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(Error::protobuf_error(e));
                    }
                };
                Ok(resp.data)
            }
            _ => Err(Error::tendermint_error(out.log)),
        }
    }

    pub fn query_wasm_contract_all(
        &self,
        address: &str,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, Error> {
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmwasm::wasm::v1::QueryAllContractStateRequest;
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmwasm::wasm::v1::QueryAllContractStateResponse;
        let request = QueryAllContractStateRequest {
            address: address.to_string(),
            pagination: None,
        };
        let path = "/cosmwasm.wasm.v1.Query/AllContractState";
        let data = serialize(&request).unwrap();
        let out = self.query_raw(path, data.as_slice()).unwrap();
        match out.code {
            abci::Code::Ok => {
                let resp = match QueryAllContractStateResponse::decode(out.value.as_slice()) {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(Error::protobuf_error(e));
                    }
                };
                let mut out = HashMap::new();
                for model in resp.models {
                    out.insert(model.key, model.value);
                }
                Ok(out)
            }
            _ => Err(Error::tendermint_error(out.log)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::contract_vm::rpc_mock::rpc::CwRpcClient;
    use cosmwasm_std::{Addr, Uint128};
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    const MALAGA_RPC_URL: &'static str = "https://rpc.malaga-420.cosmwasm.com:443";
    const MALAGA_CHAIN_ID: &'static str = "malaga-420";
    const MALAGA_BLOCK_NUMBER: u64 = 2246678;
    const EOA_ADDRESS: &'static str = "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj";
    const PAIR_ADDRESS: &'static str =
        "wasm15le5evw4regnwf9lrjnpakr2075fcyp4n4yzpelvqcuevzkw2lss46hslz";
    const TOKEN_ADDRESS: &'static str =
        "wasm124v54ngky9wxhx87t252x4xfgujmdsu7uhjdugtkkqt39nld0e6st7e64h";

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum AssetInfo {
        Token { contract_addr: String },
        NativeToken { denom: String },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    pub enum AssetInfoRaw {
        Token { contract_addr: String },
        NativeToken { denom: String },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    pub struct Asset {
        pub info: AssetInfo,
        pub amount: Uint128,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
        Pair {},
        Pool {},
        Simulation { offer_asset: Asset },
        ReverseSimulation { ask_asset: Asset },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    pub struct SimulationResponse {
        pub return_amount: Uint128,
        pub spread_amount: Uint128,
        pub commission_amount: Uint128,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    pub struct PairInfo {
        pub asset_infos: [AssetInfoRaw; 2],
        pub contract_addr: String,
        pub liquidity_token: String,
        pub asset_decimals: [u8; 2],
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[serde(rename_all = "snake_case")]
    pub struct TokenInfo {
        pub name: String,
        pub symbol: String,
        pub decimals: u8,
        pub total_supply: Uint128,
        pub mint: Option<MinterData>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub struct MinterData {
        pub minter: Addr,
        /// cap is how many more tokens can be issued by the minter
        pub cap: Option<Uint128>,
    }

    #[test]
    fn test_rpc_status() {
        let client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let chain_id = client.chain_id().unwrap();
        assert_eq!(chain_id.as_str(), MALAGA_CHAIN_ID);
    }

    #[test]
    fn test_rpc_bank() {
        let client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let balances = client.query_bank_all_balances(EOA_ADDRESS).unwrap();
        assert_eq!(balances[0].0.as_str(), "umlg");
    }

    #[test]
    fn test_rpc_contract_small() {
        let client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let qmsg = QueryMsg::Simulation {
            offer_asset: Asset {
                info: AssetInfo::NativeToken {
                    denom: "umlg".to_string(),
                },
                amount: Uint128::from(100 as u64),
            },
        };
        let qmsg_bytes = serde_json::to_vec(&qmsg).unwrap();
        let resp_bytes = client
            .query_wasm_contract_smart(PAIR_ADDRESS, qmsg_bytes.as_slice())
            .unwrap();
        let resp: SimulationResponse = serde_json::from_slice(resp_bytes.as_slice()).unwrap();
        assert_eq!(resp.return_amount.u128(), 99);
    }

    #[test]
    fn test_rpc_contract_large() {
        let client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let states_pair = client.query_wasm_contract_all(PAIR_ADDRESS).unwrap();
        let pair_info_key = Vec::from("pair_info");
        let pair_info: PairInfo =
            serde_json::from_slice(states_pair[&pair_info_key].as_slice()).unwrap();
        let states_token = client.query_wasm_contract_all(TOKEN_ADDRESS).unwrap();
        let token_info_key = Vec::from("token_info");
        let token_info: TokenInfo = serde_json::from_slice(states_token[&token_info_key].as_slice()).unwrap();
        assert_eq!(pair_info.asset_decimals[0], token_info.decimals);
    }
}
