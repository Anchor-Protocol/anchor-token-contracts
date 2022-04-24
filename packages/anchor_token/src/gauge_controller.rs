use cosmwasm_std::{Decimal, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub anchor_token: String,
    pub anchor_voting_escrow: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddGauge {
        gauge_addr: String,
        weight: Uint128,
    },
    ChangeGaugeWeight {
        gauge_addr: String,
        weight: Uint128,
    },
    VoteForGaugeWeight {
        gauge_addr: String,
        ratio: u64,
    },
    UpdateConfig {
        owner: Option<String>,
        anchor_token: Option<String>,
        anchor_voting_escrow: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GaugeCount {},
    GaugeWeight { gauge_addr: String },
    GaugeWeightAt { gauge_addr: String, time: u64 },
    TotalWeight {},
    TotalWeightAt { time: u64 },
    GaugeRelativeWeight { gauge_addr: String },
    GaugeRelativeWeightAt { gauge_addr: String, time: u64 },
    GaugeAddr { gauge_id: u64 },
    AllGaugeAddr {},
    Config {},
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeWeightResponse {
    pub gauge_weight: Uint128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeWeightAtResponse {
    pub gauge_weight_at: Uint128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct TotalWeightResponse {
    pub total_weight: Uint128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct TotalWeightAtResponse {
    pub total_weight_at: Uint128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeRelativeWeightResponse {
    pub gauge_relative_weight: Decimal,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeRelativeWeightAtResponse {
    pub gauge_relative_weight_at: Decimal,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeCountResponse {
    pub gauge_count: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeAddrResponse {
    pub gauge_addr: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct AllGaugeAddrResponse {
    pub all_gauge_addr: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub anchor_token: String,
    pub anchor_voting_escrow: String,
}

pub struct MigrateMsg {}
