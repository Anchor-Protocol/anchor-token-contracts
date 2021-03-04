use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub gov_contract: HumanAddr,   // anchor gov contract
    pub anchor_token: HumanAddr,   // anchor token address
    pub whitelist: Vec<HumanAddr>, // whitelisted contract addresses to spend distributor
    pub spend_limit: Uint128,      // spend limit per each `spend` request
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateConfig {
        spend_limit: Option<Uint128>,
    },
    Spend {
        recipient: HumanAddr,
        amount: Uint128,
    },
    AddDistributor {
        distributor: HumanAddr,
    },
    RemoveDistributor {
        distributor: HumanAddr,
    },
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub gov_contract: HumanAddr,
    pub anchor_token: HumanAddr,
    pub whitelist: Vec<HumanAddr>,
    pub spend_limit: Uint128,
}
