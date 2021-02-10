use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::HumanAddr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub gov_contract: HumanAddr, // collected rewards receiver
    pub terraswap_factory: HumanAddr,
    pub anchor_token: HumanAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// Public Message
    /// Sweep all given denom balance to ANC token 
    /// and execute Distribute message
    Sweep { denom: String },

    /// Internal Message 
    /// Distribute all ANC token to gov_contract
    Distribute {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub gov_contract: HumanAddr, // collected rewards receiver
    pub terraswap_factory: HumanAddr,
    pub anchor_token: HumanAddr,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
