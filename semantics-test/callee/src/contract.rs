#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, SubMsgResult, Response, StdResult,
    SubMsg, WasmMsg, ReplyOn, Event, Reply,
};
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{CalleeInstantiateMsg, ExecuteMsg, InstantiateMsg, QueryMsg, CalleeExecuteMsg};
use crate::state::CALLEE;

/*
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:callee";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
*/

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let mut response = Response::new();
    response = response.add_submessage(SubMsg {
        id: 1,
        gas_limit: None,
        msg: CosmosMsg::Wasm(WasmMsg::Instantiate {
            admin: None,
            code_id: msg.code_id,
            msg: to_binary(&CalleeInstantiateMsg {}).unwrap(),
            funds: vec![],
            label: "callee".to_string()
        }),
        reply_on: cosmwasm_std::ReplyOn::Always,
    });
    Ok(response)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CallRoot0 {} => execute_call_root_0(deps, env, info),
        _ => unimplemented!(),
    }
}

fn generate_call<T>(callee: &str, succeed: bool, reply_on: ReplyOn) -> SubMsg<T> {
    SubMsg {
        id: 0,
        gas_limit: None,
        msg: CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: callee.to_string(),
            msg: to_binary(&CalleeExecuteMsg::CallLeaf {
                succeed
            }).unwrap(),
            funds: vec![],
        }),
        reply_on,
    }
}

fn execute_call_root_0(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let response = Response::new();
    let mut msgs = Vec::new();

    let callee = CALLEE.load(deps.storage).unwrap();
    msgs.push(generate_call(&callee, false, ReplyOn::Always));
    msgs.push(generate_call(&callee, true, ReplyOn::Error));
    msgs.push(generate_call(&callee, false, ReplyOn::Error));
    msgs.push(generate_call(&callee, true, ReplyOn::Success));
    msgs.push(generate_call(&callee, false, ReplyOn::Success));
    Ok(response.add_submessages(msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let mut response = Response::new();
    match msg.result {
        SubMsgResult::Ok(_sub_msg_response) => {
            response = response.add_attributes(vec![
                ("caller_reply", "success"),
            ]);
        },
        SubMsgResult::Err(e) => {
            response = response.add_attributes(vec![
                ("caller_reply", e),
            ]);
        }
    }
    Ok(response)
}

#[cfg(test)]
mod tests {}
