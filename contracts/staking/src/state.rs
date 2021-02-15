use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, ReadonlyStorage, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

static KEY_CONFIG: &[u8] = b"config";
static KEY_POOL_INFO: &[u8] = b"pool_info";

static PREFIX_REWARD: &[u8] = b"reward";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub anchor_token: CanonicalAddr,
    pub staking_token: CanonicalAddr,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    pub pending_reward: Uint128, // not distributed amount due to zero bonding
    pub total_bond_amount: Uint128,
    pub reward_index: Decimal,
}

pub fn store_pool_info<S: Storage>(storage: &mut S, pool_info: &PoolInfo) -> StdResult<()> {
    singleton(storage, KEY_POOL_INFO).save(pool_info)
}

pub fn read_pool_info<S: Storage>(storage: &S) -> StdResult<PoolInfo> {
    singleton_read(storage, KEY_POOL_INFO).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}

/// returns return reward_info of the given owner
pub fn store_reward_info<S: Storage>(
    storage: &mut S,
    owner: &CanonicalAddr,
    reward_info: &RewardInfo,
) -> StdResult<()> {
    Bucket::new(PREFIX_REWARD, storage).save(owner.as_slice(), reward_info)
}

/// remove reward_info of the given owner
pub fn remove_reward_info<S: Storage>(storage: &mut S, owner: &CanonicalAddr) {
    Bucket::<S, RewardInfo>::new(PREFIX_REWARD, storage).remove(owner.as_slice())
}

/// returns rewards owned by this owner
/// (read-only version for queries)
pub fn read_reward_info<S: ReadonlyStorage>(
    storage: &S,
    owner: &CanonicalAddr,
) -> StdResult<RewardInfo> {
    match ReadonlyBucket::new(PREFIX_REWARD, storage).may_load(owner.as_slice())? {
        Some(reward_info) => Ok(reward_info),
        None => Ok(RewardInfo {
            index: Decimal::zero(),
            bond_amount: Uint128::zero(),
            pending_reward: Uint128::zero(),
        }),
    }
}
