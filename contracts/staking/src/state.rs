use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, ReadonlyStorage, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

static KEY_CONFIG: &[u8] = b"config";
static KEY_STATE: &[u8] = b"state";

static PREFIX_REWARD: &[u8] = b"reward";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub anchor_token: CanonicalAddr,
    pub staking_token: CanonicalAddr,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub last_distributed: u64,
    pub total_bond_amount: Uint128,
    pub global_reward_index: Decimal,
}

pub fn store_state<S: Storage>(storage: &mut S, state: &State) -> StdResult<()> {
    singleton(storage, KEY_STATE).save(state)
}

pub fn read_state<S: Storage>(storage: &S) -> StdResult<State> {
    singleton_read(storage, KEY_STATE).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub reward_index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}

/// returns return staker_info of the given owner
pub fn store_staker_info<S: Storage>(
    storage: &mut S,
    owner: &CanonicalAddr,
    staker_info: &StakerInfo,
) -> StdResult<()> {
    Bucket::new(PREFIX_REWARD, storage).save(owner.as_slice(), staker_info)
}

/// remove staker_info of the given owner
pub fn remove_staker_info<S: Storage>(storage: &mut S, owner: &CanonicalAddr) {
    Bucket::<S, StakerInfo>::new(PREFIX_REWARD, storage).remove(owner.as_slice())
}

/// returns rewards owned by this owner
/// (read-only version for queries)
pub fn read_staker_info<S: ReadonlyStorage>(
    storage: &S,
    owner: &CanonicalAddr,
) -> StdResult<StakerInfo> {
    match ReadonlyBucket::new(PREFIX_REWARD, storage).may_load(owner.as_slice())? {
        Some(staker_info) => Ok(staker_info),
        None => Ok(StakerInfo {
            reward_index: Decimal::zero(),
            bond_amount: Uint128::zero(),
            pending_reward: Uint128::zero(),
        }),
    }
}
