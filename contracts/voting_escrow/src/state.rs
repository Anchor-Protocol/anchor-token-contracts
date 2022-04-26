use cosmwasm_std::{Addr, CanonicalAddr, Decimal, Uint128};
use cw_storage_plus::{Item, Map, U64Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// This structure describes the main control config of voting escrow contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// contract address use for settings control
    pub owner: CanonicalAddr,
    /// ANC token address
    pub anchor_token: CanonicalAddr,
    /// min time to lock ANC for (in seconds)
    pub min_lock_time: u64,
    /// max time to lock ANC for (in seconds)
    pub max_lock_time: u64,
    /// duration of a period (in seconds) - voting power decays every period
    pub period_duration: u64,
    /// controls max boost possible (in multiples of 10. e.g: 25 = 2.5x boost)
    pub boost_coefficient: u64,
}

/// ## Description
/// This structure describes the point in checkpoints history.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Point {
    /// voting power
    pub power: Uint128,
    /// equals to the point period
    pub start: u64,
    /// the period when the lock should expire
    pub end: u64,
    /// voting power decay per period at the current period
    pub slope: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Lock {
    /// the total ANC tokens were deposited
    pub amount: Uint128,
    /// the period when lock was created
    pub start: u64,
    /// the period when the lock should expire
    pub end: u64,
    /// the last period when the lock's time was increased
    pub last_extend_lock_period: u64,
}

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// ## Description
/// Stores all user locks
pub const LOCKED: Map<Addr, Lock> = Map::new("locked");

/// ## Description
/// Stores checkpoint history per composed key (addr, period).
/// Total voting power checkpoints are stored by (contract_addr, period) key.
pub const HISTORY: Map<(Addr, U64Key), Point> = Map::new("history");

/// ## Description
/// Scheduled slope changes per period
pub const SLOPE_CHANGES: Map<U64Key, Decimal> = Map::new("slope_changes");

/// ## Description
/// Last period when scheduled slope change was applied
pub const LAST_SLOPE_CHANGE: Item<u64> = Item::new("last_slope_change");
