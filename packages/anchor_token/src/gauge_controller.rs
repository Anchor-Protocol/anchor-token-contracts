use cosmwasm_std::{Decimal, CanonicalAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub voting_escrow_contract: CanonicalAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddGauge {
        addr: CanonicalAddr,
        weight: Uint128,
    },
    ChangeGaugeWeight {
        addr: CanonicalAddr,
        weight: Uint128,
    },
    VoteForGaugeWeight {
        addr: CanonicalAddr,
        user_weight: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetNGauges {},
    GetGaugeWeight { addr: CanonicalAddr },
    GetTotalWeight {},
    GetGaugeAddr { gauge_id: u64 },
    GetConfig {},
    GetRelativeWeight { addr: CanonicalAddr, time: Uint128 },
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
pub struct NGaugesResponse {
    pub n_gauges: u64
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct GaugeAddrResponse {
    pub gauge_addr: CanonicalAddr
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub voting_escrow_contract: CanonicalAddr
}
