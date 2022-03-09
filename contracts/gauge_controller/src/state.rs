use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map, U64Key};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VotingEscrowContractQueryMsg {
    LastUserSlope { user: String },
    UserUnlockPeriod { user: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserSlopResponse {
    pub slope: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserUnlockPeriodResponse {
    pub unlock_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub anchor_token: Addr,
    pub anchor_voting_escorw: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GaugeWeight {
    pub bias: Uint128,
    pub slope: Uint128,
    pub slope_change: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GaugeInfo {
    pub last_vote_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserVote {
    pub slope: Uint128,
    pub start_period: u64,
    pub end_period: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const GAUGE_COUNT: Item<u64> = Item::new("gauge_count");

pub const GAUGE_WEIGHT: Map<(Addr, U64Key), GaugeWeight> = Map::new("gauge_weight");

pub const GAUGE_INFO: Map<Addr, GaugeInfo> = Map::new("gauge_info");

pub const GAUGE_ADDR: Map<U64Key, Addr> = Map::new("gauge_addr");

pub const USER_VOTES: Map<(Addr, Addr), UserVote> = Map::new("user_votes");
