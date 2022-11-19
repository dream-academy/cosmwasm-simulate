#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Event, MessageInfo, Response, Uint128};
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
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
    let number = Uint128::new(0x1337);
    let _ = NUMBER.save(deps.storage, &number);
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ProcessData { data1, data2 } => process_data(deps, data1, data2),
    }
}

fn process_data(deps: DepsMut, data1: String, data2: Uint128) -> Result<Response, ContractError> {
    let number = NUMBER.load(deps.storage)?;
    if number == data2 {
        Ok(Response::new().add_event(Event::new("branch1").add_attribute("value", "1")))
    } else {
        if data1.as_str() == "DreamAcademy" {
            Ok(Response::new()
                .add_event(Event::new("branch2").add_attribute("value", format!("{}", "1"))))
        } else {
            Err(ContractError::Unauthorized {})
        }
    }
}

#[cfg(test)]
mod tests {}
