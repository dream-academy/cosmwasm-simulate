#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
    WasmMsg,
};
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::NUMBER;

/*
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:flow-test-2";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
*/

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Flow {} => execute_flow(deps, env),
    }
}

pub fn execute_flow(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let value = NUMBER.load(deps.storage).unwrap_or(0);
    let new_value = value + 1;
    NUMBER.save(deps.storage, &new_value)?;
    if value < 20 {
        let msgs1 = to_binary(&ExecuteMsg::Flow {})?;
        Ok(Response::new()
            .add_attribute("action", value.to_string())
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: msgs1,
                funds: vec![],
            })))
    } else {
        Ok(Response::new().add_attribute("state", "finish"))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg(test)]
mod tests {}
