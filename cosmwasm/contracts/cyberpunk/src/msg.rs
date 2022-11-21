use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub enum ExecuteMsg {
    /// Hashes some data. Uses CPU and memory, but no external calls.
    Argon2 {
        /// The amount of memory requested (KB).
        mem_cost: u32,
        /// The number of passes.
        time_cost: u32,
    },
    /// Returns the env for testing
    MirrorEnv {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the env for testing
    #[returns(cosmwasm_std::Env)]
    MirrorEnv {},
}
