use cosmwasm_std::ContractInfo;
use cosmwasm_std::Timestamp;
use hex;
use prost::Message;
use rustbreak::{deser::Bincode, PathDatabase};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::env;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::str::FromStr;
use tendermint::abci;
use tendermint::block::Height;
use tendermint::Time;
use tendermint_rpc::endpoint::abci_query::AbciQuery;
use tendermint_rpc::{Client, HttpClient};
use tokio;

use crate::contract_vm::error::Error;

use self::rpc_items::cosmwasm::wasm;

macro_rules! include_proto {
    ($x: literal) => {
        include!(concat!(env!("OUT_DIR"), "/", $x, ".rs"));
    };
}

const RPC_CACHE_DIRNAME: &str = ".cw-rpc-cache";

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

    cache: RpcCache,
}

pub enum RpcCache {
    Empty,
    FileBacked {
        // (path: String, data: Vec<u8>) -> AbciQuery
        db: PathDatabase<HashMap<(String, Vec<u8>), AbciQuery>, Bincode>,
    },
}

fn sha256hex(input_str: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input_str.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

impl RpcCache {
    pub fn file_backed(url: &str, block_number: u64) -> Result<Self, Error> {
        let filename = sha256hex(&format!("{}||{}", url, block_number));
        let homedir = match env::var("HOME") {
            Ok(val) => val,
            Err(_) => "/tmp/".to_string(),
        };
        let cachedir = format!("{}/{}", homedir, RPC_CACHE_DIRNAME);
        let cachedir_path = Path::new(&cachedir);
        if !cachedir_path.is_dir() {
            fs::create_dir(cachedir_path).map_err(|e| Error::rpc_error(e))?;
        }
        let cachefile = format!("{}/{}", cachedir, filename);
        let cachefile_path = Path::new(&cachefile);
        println!("cache at: {}", cachefile_path.to_str().unwrap());
        Ok(Self::FileBacked {
            db: PathDatabase::load_from_path_or_default(cachefile_path.to_path_buf())
                .map_err(|e| Error::rpc_error(e))?,
        })
    }

    pub fn read(&self, path: &str, data: &[u8]) -> Result<Option<AbciQuery>, Error> {
        let key = (path.to_string(), data.to_vec());
        match self {
            // empty always returns None
            Self::Empty => Ok(None),
            Self::FileBacked { db } => match db.read(|db| db.get(&key).map(|x| x.clone())) {
                Ok(d) => Ok(d),
                Err(e) => Err(Error::rpc_error(e)),
            },
        }
    }

    pub fn write(&mut self, path: &str, data: &[u8], response: &AbciQuery) -> Result<(), Error> {
        let key = (path.to_string(), data.to_vec());
        match self {
            // empty always returns None
            Self::Empty => Ok(()),
            Self::FileBacked { db } => match db.write(|db| db.insert(key, response.clone())) {
                Ok(d) => Ok(()),
                Err(e) => Err(Error::rpc_error(e)),
            },
        }
    }
}

impl Drop for RpcCache {
    fn drop(&mut self) {
        match self {
            Self::Empty => {},
            Self::FileBacked { db } => {
                db.save().unwrap();
            }
        }
    }
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
            cache: RpcCache::Empty,
        };
        let block_height = rv.block_height()?;
        if let Some(bn) = block_number {
            if bn > block_height {
                let msg = format!("invalid block number, exceeds height({})", block_height);
                Err(Error::invalid_argument(msg))
            } else {
                rv.block_number = bn;
                rv.cache = RpcCache::file_backed(url, bn)?;
                Ok(rv)
            }
        } else {
            rv.block_number = block_height;
            rv.cache = RpcCache::file_backed(url, block_height)?;
            Ok(rv)
        }
    }

    pub fn block_number(&self) -> u64 {
        return self.block_number;
    }

    pub fn chain_id(&self) -> Result<String, Error> {
        let status = wait_future(self._inner.status())?.map_err(|e| Error::rpc_error(e))?;
        Ok(status.node_info.network.to_string())
    }

    /// returns timestamp of self.block_number
    pub fn timestamp(&self) -> Result<Timestamp, Error> {
        let block_info =
            wait_future(self._inner.block(
                Height::try_from(self.block_number).map_err(|e| Error::tendermint_error(e))?,
            ))?
            .map_err(|e| Error::rpc_error(e))?;
        let time = block_info.block.header.time;
        let duration = time
            .duration_since(Time::unix_epoch())
            .map_err(|e| Error::tendermint_error(e))?;
        Ok(Timestamp::from_nanos(
            duration
                .as_nanos()
                .try_into()
                .map_err(|e| Error::tendermint_error(e))?,
        ))
    }

    pub fn block_height(&self) -> Result<u64, Error> {
        let status = wait_future(self._inner.status())?.map_err(|e| Error::rpc_error(e))?;
        Ok(status.sync_info.latest_block_height.value())
    }

    pub fn query_raw(&mut self, path_: &str, data: &[u8]) -> Result<AbciQuery, Error> {
        if let Some(in_db) = self.cache.read(path_, data)? {
            return Ok(in_db);
        }
        let path = match abci::Path::from_str(path_) {
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
        let result = wait_future(
            self._inner
                .abci_query(Some(path), data, Some(height), false),
        )?
        .map_err(|e| Error::rpc_error(e))?;
        self.cache.write(path_, data, &result);
        Ok(result)
    }

    pub fn query_bank_all_balances(&mut self, address: &str) -> Result<Vec<(String, u128)>, Error> {
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
                let balances: Vec<(String, u128)> = resp
                    .balances
                    .iter()
                    .map(|x| (x.denom.to_string(), u128::from_str(&x.amount).unwrap()))
                    .collect();
                Ok(balances)
            }
            _ => Err(Error::tendermint_error(out.log)),
        }
    }

    pub fn query_wasm_contract_smart(
        &mut self,
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
        &mut self,
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

    pub fn query_wasm_contract_info(&mut self, address: &str) -> Result<wasm::v1::ContractInfo, Error> {
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmwasm::wasm::v1::QueryContractInfoRequest;
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmwasm::wasm::v1::QueryContractInfoResponse;
        let request = QueryContractInfoRequest {
            address: address.to_string(),
        };
        let path = "/cosmwasm.wasm.v1.Query/ContractInfo";
        let data = serialize(&request).unwrap();
        let out = self.query_raw(path, data.as_slice()).unwrap();
        match out.code {
            abci::Code::Ok => {
                let resp = match QueryContractInfoResponse::decode(out.value.as_slice()) {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(Error::protobuf_error(e));
                    }
                };
                if let Some(ci) = resp.contract_info {
                    Ok(ci)
                } else {
                    Err(Error::invalid_argument(format!(
                        "address {} is most likely not a contract address",
                        address
                    )))
                }
            }
            _ => Err(Error::tendermint_error(out.log)),
        }
    }

    pub fn query_wasm_contract_code(&mut self, code_id: u64) -> Result<Vec<u8>, Error> {
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmwasm::wasm::v1::QueryCodeRequest;
        use crate::contract_vm::rpc_mock::rpc::rpc_items::cosmwasm::wasm::v1::QueryCodeResponse;
        let request = QueryCodeRequest { code_id: code_id };
        let path = "/cosmwasm.wasm.v1.Query/Code";
        let data = serialize(&request).unwrap();
        let out = self.query_raw(path, data.as_slice()).unwrap();
        match out.code {
            abci::Code::Ok => {
                let resp = match QueryCodeResponse::decode(out.value.as_slice()) {
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
        let mut client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let balances = client.query_bank_all_balances(EOA_ADDRESS).unwrap();
        assert_eq!(balances[0].0.as_str(), "umlg");
    }

    #[test]
    fn test_rpc_contract_small() {
        let mut client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
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
        let mut client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let states_pair = client.query_wasm_contract_all(PAIR_ADDRESS).unwrap();
        let pair_info_key = Vec::from("pair_info");
        let pair_info: PairInfo =
            serde_json::from_slice(states_pair[&pair_info_key].as_slice()).unwrap();
        let states_token = client.query_wasm_contract_all(TOKEN_ADDRESS).unwrap();
        let token_info_key = Vec::from("token_info");
        let token_info: TokenInfo =
            serde_json::from_slice(states_token[&token_info_key].as_slice()).unwrap();
        assert_eq!(pair_info.asset_decimals[0], token_info.decimals);
    }

    #[test]
    fn test_rpc_get_code() {
        let mut client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
        let contract_info = client.query_wasm_contract_info(PAIR_ADDRESS).unwrap();
        assert_eq!(contract_info.code_id, 1786);
        let wasm_code = client
            .query_wasm_contract_code(contract_info.code_id)
            .unwrap();
        // wasm header is \x00asm, for some contracts it may be gzip
        assert_eq!(&wasm_code[0..4], &vec![0, 97, 115, 109]);
        let wasm_code = client.query_wasm_contract_code(1).unwrap();
        println!("{:?}", &wasm_code[0..4]);
    }
}
