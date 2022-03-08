use cosmwasm_std::{Decimal, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub anchor_token: String,
    pub anchor_voting_escorw: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddGauge {
        addr: String,
        weight: Uint128,
    },
    ChangeGaugeWeight {
        addr: String,
        weight: Uint128,
    },
    VoteForGaugeWeight {
        addr: String,
        user_weight: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GaugeCount {},
    GaugeWeight { addr: String },
    TotalWeight {},
    GaugeAddr { gauge_id: u64 },
    AllGaugeAddr {},
    Config {},
    RelativeWeight { addr: String, time: Uint128 },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeWeightResponse {
    pub gauge_weight: Uint128
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct TotalWeightResponse {
    pub total_weight: Uint128
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct RelativeWeightResponse {
    pub relative_weight: Decimal
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeCountResponse {
    pub gauge_count: u64
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeAddrResponse {
    pub gauge_addr: String
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct AllGaugeAddrResponse {
    pub all_gauge_addr: Vec<String>
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub anchor_token: String,
    pub anchor_voting_escorw: String,
}
