use cosmwasm_std::{Decimal, Uint128};
use cw20::Logo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// This structure describes marketing info.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMarketingInfo {
    pub project: Option<String>,
    pub description: Option<String>,
    pub marketing: Option<String>,
    pub logo: Option<Logo>,
}

/// ## Description
/// This structure describes the basic settings for creating a contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// contract owner
    pub owner: String,
    /// ANC token address
    pub anchor_token: String,
    /// min time to lock ANC for (in seconds)
    pub min_lock_time: u64,
    /// max time to lock ANC for (in seconds)
    pub max_lock_time: u64,
    /// duration of a period (in seconds) - voting power decays every period
    pub period_duration: u64,
    /// controls max boost possible (in multiples of 10. e.g: 25 = 2.5x boost)
    pub boost_coefficient: u64,
    /// Marketing info
    pub marketing: Option<InstantiateMarketingInfo>,
    
}

/// ## Description
/// This structure describes the execute messages of the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ExtendLockAmount {
        user: String,
        amount: Uint128,
    },
    ExtendLockTime {
        user: String,
        time: u64,
    },
    /// Receives a message of type [`Cw20ReceiveMsg`] and processes it depending on the received
    /// template.
    Withdraw {
        user: String,
        amount: Uint128,
    },
    UpdateMarketing {
        /// A URL pointing to the project behind this token.
        project: Option<String>,
        /// A longer description of the token and it's utility. Designed for tooltips or such
        description: Option<String>,
        /// The address (if any) who can update this data structure
        marketing: Option<String>,
    },
    UploadLogo(Logo),
    UpdateConfig {
        owner: Option<String>,
        anchor_token: Option<String>,
    },
}

/// ## Description
/// This structure describes the query messages of the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    TotalVotingPower {},
    TotalVotingPowerAt { time: u64 },
    TotalVotingPowerAtPeriod { period: u64 },
    UserVotingPower { user: String },
    UserVotingPowerAt { user: String, time: u64 },
    UserVotingPowerAtPeriod { user: String, period: u64 },
    LastUserSlope { user: String },
    UserUnlockPeriod { user: String },
    LockInfo { user: String },
    MarketingInfo {},
    DownloadLogo {},
    Config {},
    TokenInfo {},
}

/// ## Description
/// This structure describes voting power response.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct VotingPowerResponse {
    pub voting_power: Uint128,
}

/// ## Description
/// This structure describes last user slope response.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserSlopeResponse {
    pub slope: Decimal,
}

/// ## Description
/// This structure describes user unlock period (lock end).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserUnlockPeriodResponse {
    pub unlock_period: u64,
}

/// ## Description
/// This structure describes lock information response.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockInfoResponse {
    pub amount: Uint128,
    pub coefficient: Decimal,
    pub start: u64,
    pub end: u64,
}

/// ## Description
/// This structure describes config response.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub anchor_token: String,
    pub min_lock_time: u64,
    pub max_lock_time: u64,
    pub period_duration: u64,
    pub boost_coefficient: u64,
}

pub struct MigrateMsg {}
