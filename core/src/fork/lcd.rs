use super::client_backend::ContractInfo;
use crate::{CwClientBackend, Error};
use chrono::DateTime;
use cosmwasm_std::Timestamp;
use oxhttp::model::{Method, Request, Status, Url};
use oxhttp::Client;
use serde::{Deserialize, Serialize};
use serde_json::from_str;

use std::collections::BTreeMap;

#[derive(Clone)]
pub struct CwLcdClient {
    url: String,
    block_number: u64,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct BlockHeaderRawOuterOuter {
    block: BlockHeaderRawOuter,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct BlockHeaderRawOuter {
    header: BlockHeaderRaw,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct BlockHeaderRaw {
    chain_id: String,
    height: String,
    time: String,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct ContractInfoResponse {
    address: String,
    contract_info: ContractInfoRaw,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct ContractInfoRaw {
    code_id: String,
    creator: String,
    admin: String,
    label: String,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct ContractStateAll {
    models: Vec<KeyValueEntry>,
}

#[derive(Serialize, Deserialize)]
struct KeyValueEntry {
    key: String,
    value: String,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct CodeResponse {
    data: String,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct BankBalancesResponse {
    balances: Vec<CoinRaw>,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct CoinRaw {
    denom: String,
    amount: String,
}

// never change the field names of this struct
#[derive(Serialize, Deserialize)]
struct ErrorResponseBody {
    code: String,
    message: String,
}

impl CwLcdClient {
    pub fn new(url: &str) -> Result<Self, Error> {
        let mut rv = Self {
            url: url.to_string(),
            block_number: 0,
        };
        rv.block_number = rv.block_height()?;
        Ok(rv)
    }

    fn request_inner(&self, uri: &str) -> Result<String, Error> {
        let request_url =
            Url::parse(&format!("{}{}", &self.url, uri)).map_err(Error::format_error)?;
        let client = Client::new();
        let request = Request::builder(Method::GET, request_url).build();
        let response = client.request(request).map_err(Error::http_error)?;
        let status = response.status();
        let body_str = response
            .into_body()
            .to_string()
            .map_err(Error::http_error)?;
        if status == Status::OK {
            Ok(body_str)
        } else {
            let err_body: ErrorResponseBody = from_str(&body_str).map_err(Error::format_error)?;
            Err(Error::http_error(&err_body.message))
        }
    }

    fn get_latest_block_header(&mut self) -> Result<BlockHeaderRaw, Error> {
        let body_str = self.request_inner("/blocks/latest")?;
        let block_header: BlockHeaderRawOuterOuter =
            from_str(&body_str).map_err(Error::format_error)?;
        Ok(block_header.block.header)
    }
}

impl CwClientBackend for CwLcdClient {
    fn block_number(&self) -> u64 {
        self.block_number
    }

    fn chain_id(&mut self) -> Result<String, crate::Error> {
        let block_header = self.get_latest_block_header()?;
        Ok(block_header.chain_id)
    }

    fn timestamp(&mut self) -> Result<Timestamp, crate::Error> {
        let block_header = self.get_latest_block_header()?;
        let date_time =
            DateTime::parse_from_rfc3339(&block_header.time).map_err(Error::format_error)?;
        Ok(Timestamp::from_nanos(date_time.timestamp_nanos() as u64))
    }

    fn block_height(&mut self) -> Result<u64, crate::Error> {
        let block_header = self.get_latest_block_header()?;
        let height: u64 = block_header.height.parse().unwrap();
        Ok(height)
    }

    fn query_bank_all_balances(
        &mut self,
        address: &str,
    ) -> Result<Vec<(String, u128)>, crate::Error> {
        let body_str = self.request_inner(&format!("/cosmos/bank/v1beta1/balances/{}", address))?;
        let balances: BankBalancesResponse = from_str(&body_str).map_err(Error::format_error)?;
        let mut out = Vec::new();
        for coin in balances.balances {
            out.push((coin.denom, coin.amount.parse().unwrap()));
        }
        Ok(out)
    }

    fn query_wasm_contract_smart(
        &mut self,
        address: &str,
        query_data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let query_data_b64 = base64::encode(query_data);
        let body_str = self.request_inner(&format!(
            "/cosmwasm/wasm/v1/contract/{}/smart/{}",
            address, query_data_b64
        ))?;
        let response: serde_json::Value = from_str(&body_str).map_err(Error::format_error)?;
        if let Some(data) = response.get("data") {
            Ok(data.to_string().as_bytes().to_vec())
        } else {
            Err(Error::format_error(
                &"key 'data' not present in response".to_string(),
            ))
        }
    }

    fn query_wasm_contract_state_all(
        &mut self,
        address: &str,
    ) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, crate::Error> {
        let body_str =
            self.request_inner(&format!("/cosmwasm/wasm/v1/contract/{}/state", address))?;
        let response: ContractStateAll = from_str(&body_str).map_err(Error::format_error)?;
        let mut out = BTreeMap::new();
        for kv in response.models {
            let key = hex::decode(kv.key).map_err(Error::format_error)?;
            let value = base64::decode(kv.value).map_err(Error::format_error)?;
            out.insert(key, value);
        }
        Ok(out)
    }

    fn query_wasm_contract_info(&mut self, address: &str) -> Result<ContractInfo, crate::Error> {
        let body_str = self.request_inner(&format!("/cosmwasm/wasm/v1/contract/{}", address))?;
        let response: ContractInfoResponse = from_str(&body_str).map_err(Error::format_error)?;
        Ok(ContractInfo {
            code_id: response.contract_info.code_id.parse().unwrap(),
        })
    }

    fn query_wasm_contract_code(&mut self, code_id: u64) -> Result<Vec<u8>, crate::Error> {
        let body_str = self.request_inner(&format!("/cosmwasm/wasm/v1/code/{}", code_id))?;
        let response: CodeResponse = from_str(&body_str).map_err(Error::format_error)?;
        let code = base64::decode(&response.data).map_err(Error::format_error)?;
        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::CanonicalAddr;
    use serde::{Deserialize, Serialize};

    use super::{CwClientBackend, CwLcdClient};
    const TERRASWAP_FACTORY_ADDRESS: &str =
        "terra1466nf3zuxpya8q9emxukd7vftaf6h4psr0a07srl5zw74zh84yjqxl5qul";

    #[derive(Serialize, Deserialize, Debug)]
    pub struct TerraswapFactoryConfig {
        pub owner: CanonicalAddr,
        pub pair_code_id: u64,
        pub token_code_id: u64,
    }

    #[test]
    fn test_lcd_basic() {
        let mut lcd_client = CwLcdClient::new("https://phoenix-lcd.terra.dev").unwrap();
        assert!(lcd_client.block_number() > 2529402);
        assert!(lcd_client.timestamp().unwrap().nanos() > 1668950758945436944);

        let code = lcd_client.query_wasm_contract_code(1).unwrap();
        assert_eq!(&code[0..4], &[0, 97, 115, 109]);

        let code_info = lcd_client
            .query_wasm_contract_info(TERRASWAP_FACTORY_ADDRESS)
            .unwrap();
        // in the docs(https://docs.terraswap.io/docs/contract_resources/contract_addresses/) it states that terraswap factory has code id of 4
        // however, this value can be incremented due to migrations
        assert!(code_info.code_id > 1);

        let contract_state = lcd_client
            .query_wasm_contract_state_all(TERRASWAP_FACTORY_ADDRESS)
            .unwrap();
        assert!(contract_state.contains_key(&"config".as_bytes().to_vec()));

        let query_data = r#"
        { "config": {} }
        "#
        .as_bytes();
        let config: TerraswapFactoryConfig = serde_json::from_slice(
            &lcd_client
                .query_wasm_contract_smart(TERRASWAP_FACTORY_ADDRESS, query_data)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(config.token_code_id, 4);

        let balances = lcd_client
            .query_bank_all_balances(TERRASWAP_FACTORY_ADDRESS)
            .unwrap();
        assert!(!balances.is_empty());
    }
}
