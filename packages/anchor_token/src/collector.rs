use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_bignumber::Decimal256;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub gov_contract: String, // collected rewards receiver
    pub terraswap_factory: String,
    pub anchor_token: String,
    pub reward_factor: Decimal256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Update config interface
    /// to enable reward_factor update
    UpdateConfig { reward_factor: Option<Decimal256> },
    /// Public Message
    /// Sweep all given denom balance to ANC token
    /// and execute Distribute message
    Sweep { denom: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub gov_contract: String, // collected rewards receiver
    pub terraswap_factory: String,
    pub anchor_token: String,
    pub reward_factor: Decimal256,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
