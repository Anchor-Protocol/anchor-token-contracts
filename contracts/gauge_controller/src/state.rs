use cosmwasm_std::{CanonicalAddr, Storage, Uint128};

use cosmwasm_storage::{
    singleton, singleton_read, Bucket, ReadonlyBucket, ReadonlySingleton, Singleton,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static KEY_CONFIG: &[u8] = b"config";
static KEY_GAUGE_COUNT: &[u8] = b"gauge_count";

static PREFIX_GAUGE_ADDR: &[u8] = b"gauge_addr";
static PREFIX_GAUGE_INFO: &[u8] = b"gauge_info";
static PREFIX_USER_VOTES: &[u8] = b"user_votes";
static PREFIX_GAGUE_WEIGHT: &[u8] = b"gauge_weight";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub anchor_token: CanonicalAddr,
    pub anchor_voting_escorw: CanonicalAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Weight {
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

pub fn config_store(storage: &mut dyn Storage) -> Singleton<Config> {
    singleton(storage, KEY_CONFIG)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<Config> {
    singleton_read(storage, KEY_CONFIG)
}

pub fn gauge_count_store(storage: &mut dyn Storage) -> Singleton<u64> {
    singleton(storage, KEY_GAUGE_COUNT)
}

pub fn gauge_count_read(storage: &dyn Storage) -> ReadonlySingleton<u64> {
    singleton_read(storage, KEY_GAUGE_COUNT)
}

pub fn gauge_weight_store<'a>(
    storage: &'a mut dyn Storage,
    gauge_addr: &'a CanonicalAddr,
) -> Bucket<'a, Weight> {
    Bucket::multilevel(storage, &[PREFIX_GAGUE_WEIGHT, &gauge_addr])
}

pub fn gauge_weight_read<'a>(
    storage: &'a dyn Storage,
    gauge_addr: &'a CanonicalAddr,
) -> ReadonlyBucket<'a, Weight> {
    ReadonlyBucket::multilevel(storage, &[PREFIX_GAGUE_WEIGHT, &gauge_addr])
}

pub fn gauge_info_store(storage: &mut dyn Storage) -> Bucket<GaugeInfo> {
    Bucket::multilevel(storage, &[PREFIX_GAUGE_INFO])
}

pub fn gauge_info_read(storage: &dyn Storage) -> ReadonlyBucket<GaugeInfo> {
    ReadonlyBucket::multilevel(storage, &[PREFIX_GAUGE_INFO])
}

pub fn gauge_addr_store(storage: &mut dyn Storage) -> Bucket<CanonicalAddr> {
    Bucket::multilevel(storage, &[PREFIX_GAUGE_ADDR])
}

pub fn gauge_addr_read(storage: &dyn Storage) -> ReadonlyBucket<CanonicalAddr> {
    ReadonlyBucket::multilevel(storage, &[PREFIX_GAUGE_ADDR])
}
