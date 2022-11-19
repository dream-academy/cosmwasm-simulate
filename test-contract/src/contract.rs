#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo, Response, StdResult,
};
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReadNumberResponse};
use crate::state::NUMBER;

/*
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-semantics-test";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
*/

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    NUMBER.save(deps.storage, &1)?;
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::TestQuerySelf {} => execute_write_and_query_self(deps, env),
        ExecuteMsg::TestAtomic {} => execute_write_and_panic(deps),
    }
}

fn execute_write_and_query_self(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    NUMBER.save(deps.storage, &2)?;
    let msg = QueryMsg::ReadNumber {};
    let res: ReadNumberResponse = deps
        .querier
        .query_wasm_smart(env.contract.address, &msg)
        .unwrap();
    NUMBER.save(deps.storage, &1)?;
    Ok(Response::new()
        .add_event(Event::new("read_number").add_attribute("value", format!("{}", res.value))))
}

fn execute_write_and_panic(deps: DepsMut) -> Result<Response, ContractError> {
    NUMBER.save(deps.storage, &100)?;
    Err(ContractError::Unauthorized {})
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ReadNumber {} => {
            let number = NUMBER.load(deps.storage).unwrap();
            Ok(to_binary(&ReadNumberResponse { value: number }).unwrap())
        }
    }
}

#[cfg(test)]
mod tests {}
