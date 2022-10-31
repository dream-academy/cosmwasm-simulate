use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    pub code_id: u64,
}

#[cw_serde]
pub struct CalleeInstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    CallRoot0 {},
}

#[cw_serde]
pub enum CalleeExecuteMsg {
    CallLeaf {
        succeed: bool,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}
