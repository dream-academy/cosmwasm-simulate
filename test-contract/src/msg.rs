use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    TestQuerySelf {},
    TestAtomic {},
}

#[cw_serde]
pub enum QueryMsg {
    ReadNumber {},
}

#[cw_serde]
pub struct ReadNumberResponse {
    pub value: u32,
}
