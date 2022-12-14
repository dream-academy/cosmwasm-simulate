use bincode;
use cosmwasm_std::Timestamp;
use hex;
use prost::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Eq;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::future::Future;
use std::hash::Hash;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::str::FromStr;
use tendermint::abci;
use tendermint::block::Height;
use tendermint::Time;
use tendermint_rpc::{Client, HttpClient};
use tokio;

use super::client_backend::ContractInfo;
use crate::CwClientBackend;
use crate::Error;

const RPC_CACHE_DIRNAME: &str = ".cw-rpc-cache";

fn rwopen<P: AsRef<Path>>(path: P) -> std::io::Result<fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
}

fn sha256hex(input_str: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input_str.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[derive(Clone)]
pub struct CwRpcClient {
    _inner: HttpClient,
    block_number: u64,

    cache: RpcCache,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash, Clone)]
pub struct RpcCacheK {
    path: String,
    data: Vec<u8>,
}

pub type RpcCacheV = Vec<u8>;

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct RpcCacheInner {
    db: HashMap<RpcCacheK, RpcCacheV>,
    chain_id: String,
    timestamp: u64,
}

pub enum RpcCache {
    Empty,
    FileBacked {
        // (path: String, data: Vec<u8>) -> AbciQuery.value
        inner: RpcCacheInner,
        initialized: bool,
        file_name: String,
        file: fs::File,
    },
}

impl Clone for RpcCache {
    fn clone(&self) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::FileBacked {
                inner, file_name, ..
            } => Self::FileBacked {
                inner: inner.clone(),
                file_name: file_name.clone(),
                file: rwopen(file_name).unwrap(),
                initialized: true,
            },
        }
    }
}

impl RpcCache {
    fn file_backed(url: &str, block_number: u64) -> Result<Self, Error> {
        let filename = sha256hex(&format!("{}||{}", url, block_number));
        let homedir = match env::var("HOME") {
            Ok(val) => val,
            Err(_) => "/tmp/".to_string(),
        };
        let cachedir = format!("{}/{}", homedir, RPC_CACHE_DIRNAME);
        let cachedir_path = Path::new(&cachedir);
        if !cachedir_path.is_dir() {
            fs::create_dir(cachedir_path).map_err(Error::io_error)?;
        }
        let cachefile = format!("{}/{}", cachedir, filename);
        let cachefile_path = Path::new(&cachefile);
        let (file, inner, initialized) = if cachefile_path.is_file() {
            let mut file = rwopen(cachefile_path).map_err(Error::io_error)?;
            let mut file_contents = Vec::new();
            let _ = file
                .read_to_end(&mut file_contents)
                .map_err(Error::io_error)?;
            let inner: RpcCacheInner =
                bincode::deserialize(&file_contents).map_err(Error::format_error)?;
            (file, inner, true)
        } else {
            let file = rwopen(cachefile_path).map_err(Error::io_error)?;
            (file, RpcCacheInner::default(), false)
        };
        Ok(Self::FileBacked {
            inner,
            file_name: cachefile,
            file,
            initialized,
        })
    }

    fn read(&self, path: &str, data: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let key = RpcCacheK {
            path: path.to_string(),
            data: data.to_vec(),
        };
        match self {
            // empty always returns None
            Self::Empty => Ok(None),
            Self::FileBacked { inner, .. } => Ok(inner.db.get(&key).cloned()),
        }
    }

    fn chain_id(&self) -> Option<String> {
        match self {
            Self::FileBacked { inner, .. } => Some(inner.chain_id.clone()),
            Self::Empty => None,
        }
    }

    fn timestamp(&self) -> Option<u64> {
        match self {
            Self::FileBacked { inner, .. } => Some(inner.timestamp),
            Self::Empty => None,
        }
    }

    fn write(&mut self, path: &str, data: &[u8], response: &[u8]) -> Result<(), Error> {
        let key = RpcCacheK {
            path: path.to_string(),
            data: data.to_vec(),
        };
        match self {
            // empty always returns None
            Self::Empty => Ok(()),
            Self::FileBacked { inner, .. } => {
                inner.db.insert(key, response.to_owned());
                Ok(())
            }
        }
    }

    fn save(&mut self) -> Result<(), Error> {
        match self {
            Self::Empty => Ok(()),
            Self::FileBacked { inner, file, .. } => {
                let serialized = bincode::serialize(inner).map_err(Error::format_error)?;
                file.seek(SeekFrom::Start(0)).map_err(Error::io_error)?;
                file.write(&serialized).map_err(Error::io_error)?;
                Ok(())
            }
        }
    }

    fn initialized(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::FileBacked { initialized, .. } => *initialized,
        }
    }

    fn set_chain_id(&mut self, chain_id: String) {
        match self {
            Self::FileBacked { inner, .. } => inner.chain_id = chain_id,
            Self::Empty => {}
        }
    }

    fn set_timestamp(&mut self, timestamp: u64) {
        match self {
            Self::FileBacked { inner, .. } => inner.timestamp = timestamp,
            Self::Empty => {}
        }
    }
}

impl Drop for RpcCache {
    fn drop(&mut self) {
        let _ = self.save();
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
        if let Some(bn) = block_number {
            // first check if cache exists
            rv.cache = RpcCache::file_backed(url, bn)?;
            if !rv.cache.initialized() {
                let timestamp = rv.timestamp()?;
                let chain_id = rv.chain_id()?;
                rv.cache.set_chain_id(chain_id);
                rv.cache.set_timestamp(timestamp.nanos());
            }
            Ok(rv)
        } else {
            let block_height = rv.block_height()?;
            let chain_id = rv.chain_id()?;
            rv.block_number = block_height;
            // Don't change this line's order. To fetch the timestamp block_number must be properly initialized
            let timestamp = rv.timestamp()?;
            rv.cache = RpcCache::file_backed(url, block_height)?;
            rv.cache.set_chain_id(chain_id);
            rv.cache.set_timestamp(timestamp.nanos());
            Ok(rv)
        }
    }

    pub fn abci_query_raw(&mut self, path_: &str, data: &[u8]) -> Result<Vec<u8>, Error> {
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
        .map_err(Error::rpc_error)?;
        match result.code {
            abci::Code::Ok => {}
            _ => {
                return Err(Error::tendermint_error(result.log));
            }
        }
        self.cache.write(path_, data, &result.value)?;
        Ok(result.value)
    }
}

// protobuf serialize
fn serialize<M: Message>(m: &M) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    match m.encode(&mut out) {
        Ok(_) => Ok(out),
        Err(e) => Err(Error::format_error(e)),
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

impl CwClientBackend for CwRpcClient {
    fn block_number(&self) -> u64 {
        self.block_number
    }

    fn chain_id(&mut self) -> Result<String, Error> {
        if let Some(chain_id) = self.cache.chain_id() {
            Ok(chain_id)
        } else {
            let status = wait_future(self._inner.status())?.map_err(Error::rpc_error)?;
            Ok(status.node_info.network.to_string())
        }
    }

    /// returns timestamp of self.block_number
    fn timestamp(&mut self) -> Result<Timestamp, Error> {
        if let Some(timestamp_ns) = self.cache.timestamp() {
            Ok(Timestamp::from_nanos(timestamp_ns))
        } else {
            let block_info = wait_future(
                self._inner
                    .block(Height::try_from(self.block_number).map_err(Error::tendermint_error)?),
            )?
            .map_err(Error::rpc_error)?;
            let time = block_info.block.header.time;
            let duration = time
                .duration_since(Time::unix_epoch())
                .map_err(Error::tendermint_error)?;
            Ok(Timestamp::from_nanos(
                duration
                    .as_nanos()
                    .try_into()
                    .map_err(Error::tendermint_error)?,
            ))
        }
    }

    fn block_height(&mut self) -> Result<u64, Error> {
        let status = wait_future(self._inner.status())?.map_err(Error::rpc_error)?;
        Ok(status.sync_info.latest_block_height.value())
    }

    fn query_bank_all_balances(&mut self, address: &str) -> Result<Vec<(String, u128)>, Error> {
        use crate::rpc_items::cosmos::bank::v1beta1::QueryAllBalancesRequest;
        use crate::rpc_items::cosmos::bank::v1beta1::QueryAllBalancesResponse;
        let request = QueryAllBalancesRequest {
            address: address.to_string(),
            pagination: None,
        };
        let path = "/cosmos.bank.v1beta1.Query/AllBalances";
        let data = serialize(&request).unwrap();
        let out = self.abci_query_raw(path, data.as_slice())?;
        let resp = match QueryAllBalancesResponse::decode(out.as_slice()) {
            Ok(r) => r,
            Err(e) => {
                return Err(Error::format_error(e));
            }
        };
        let balances: Vec<(String, u128)> = resp
            .balances
            .iter()
            .map(|x| (x.denom.to_string(), u128::from_str(&x.amount).unwrap()))
            .collect();
        Ok(balances)
    }

    fn query_wasm_contract_smart(
        &mut self,
        address: &str,
        query_data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        use crate::rpc_items::cosmwasm::wasm::v1::QuerySmartContractStateRequest;
        use crate::rpc_items::cosmwasm::wasm::v1::QuerySmartContractStateResponse;
        let request = QuerySmartContractStateRequest {
            address: address.to_string(),
            query_data: query_data.to_vec(),
        };
        let path = "/cosmwasm.wasm.v1.Query/SmartContractState";
        let data = serialize(&request).unwrap();
        let out = self.abci_query_raw(path, data.as_slice())?;
        let resp = match QuerySmartContractStateResponse::decode(out.as_slice()) {
            Ok(r) => r,
            Err(e) => {
                return Err(Error::format_error(e));
            }
        };
        Ok(resp.data)
    }

    fn query_wasm_contract_state_all(
        &mut self,
        address: &str,
    ) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, Error> {
        use crate::rpc_items::cosmwasm::wasm::v1::QueryAllContractStateRequest;
        use crate::rpc_items::cosmwasm::wasm::v1::QueryAllContractStateResponse;
        let request = QueryAllContractStateRequest {
            address: address.to_string(),
            pagination: None,
        };
        let path = "/cosmwasm.wasm.v1.Query/AllContractState";
        let data = serialize(&request).unwrap();
        let out = self.abci_query_raw(path, data.as_slice())?;
        let resp = match QueryAllContractStateResponse::decode(out.as_slice()) {
            Ok(r) => r,
            Err(e) => {
                return Err(Error::format_error(e));
            }
        };
        let mut out = BTreeMap::new();
        for model in resp.models {
            out.insert(model.key, model.value);
        }
        Ok(out)
    }

    fn query_wasm_contract_info(&mut self, address: &str) -> Result<ContractInfo, Error> {
        use crate::rpc_items::cosmwasm::wasm::v1::QueryContractInfoRequest;
        use crate::rpc_items::cosmwasm::wasm::v1::QueryContractInfoResponse;
        let request = QueryContractInfoRequest {
            address: address.to_string(),
        };
        let path = "/cosmwasm.wasm.v1.Query/ContractInfo";
        let data = serialize(&request).unwrap();
        let out = self.abci_query_raw(path, data.as_slice())?;
        let resp = match QueryContractInfoResponse::decode(out.as_slice()) {
            Ok(r) => r,
            Err(e) => {
                return Err(Error::format_error(e));
            }
        };
        if let Some(ci) = resp.contract_info {
            Ok(ContractInfo {
                code_id: ci.code_id,
            })
        } else {
            Err(Error::invalid_argument(format!(
                "address {} is most likely not a contract address",
                address
            )))
        }
    }

    fn query_wasm_contract_code(&mut self, code_id: u64) -> Result<Vec<u8>, Error> {
        use crate::rpc_items::cosmwasm::wasm::v1::QueryCodeRequest;
        use crate::rpc_items::cosmwasm::wasm::v1::QueryCodeResponse;
        let request = QueryCodeRequest { code_id };
        let path = "/cosmwasm.wasm.v1.Query/Code";
        let data = serialize(&request).unwrap();
        let out = self.abci_query_raw(path, data.as_slice())?;
        let resp = match QueryCodeResponse::decode(out.as_slice()) {
            Ok(r) => r,
            Err(e) => {
                return Err(Error::format_error(e));
            }
        };
        Ok(resp.data)
    }
}

#[cfg(test)]
mod tests {
    use crate::{CwClientBackend, CwRpcClient};
    use cosmwasm_std::{Addr, Uint128};
    use serde::{Deserialize, Serialize};

    use super::RpcCache;

    const MALAGA_RPC_URL: &str = "https://rpc.malaga-420.cosmwasm.com:443";
    const MALAGA_CHAIN_ID: &str = "malaga-420";
    const MALAGA_BLOCK_NUMBER: u64 = 2346678;
    const EOA_ADDRESS: &str = "wasm1zcnn5gh37jxg9c6dp4jcjc7995ae0s5f5hj0lj";
    const PAIR_ADDRESS: &str = "wasm15le5evw4regnwf9lrjnpakr2075fcyp4n4yzpelvqcuevzkw2lss46hslz";
    const TOKEN_ADDRESS: &str = "wasm124v54ngky9wxhx87t252x4xfgujmdsu7uhjdugtkkqt39nld0e6st7e64h";

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

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
    #[serde(rename_all = "snake_case")]
    pub struct TokenInfo {
        pub name: String,
        pub symbol: String,
        pub decimals: u8,
        pub total_supply: Uint128,
        pub mint: Option<MinterData>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
    pub struct MinterData {
        pub minter: Addr,
        /// cap is how many more tokens can be issued by the minter
        pub cap: Option<Uint128>,
    }

    #[test]
    fn test_rpc_status() {
        let mut client = CwRpcClient::new(MALAGA_RPC_URL, Some(MALAGA_BLOCK_NUMBER)).unwrap();
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
                amount: Uint128::from(100_u64),
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
        let states_pair = client.query_wasm_contract_state_all(PAIR_ADDRESS).unwrap();
        let pair_info_key = Vec::from("pair_info");
        let pair_info: PairInfo =
            serde_json::from_slice(states_pair[&pair_info_key].as_slice()).unwrap();
        let states_token = client.query_wasm_contract_state_all(TOKEN_ADDRESS).unwrap();
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
    }

    #[test]
    fn test_cache() {
        let mut cache = RpcCache::file_backed(MALAGA_RPC_URL, 100000).unwrap();
        let path = "aaaaaaaa";
        let data = "bbbbbbbb".as_bytes();
        let response = "cccccccc".as_bytes();
        cache.write(path, data, response).unwrap();
        drop(cache);

        let cache = RpcCache::file_backed(MALAGA_RPC_URL, 100000).unwrap();
        let data = cache.read(path, data).unwrap();
        println!("{:?}", &data);
    }
}
