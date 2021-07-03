use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

static KEY_CONFIG: &[u8] = b"config";
static KEY_LATEST_STAGE: &[u8] = b"latest_stage";

static PREFIX_MERKLE_ROOT: &[u8] = b"merkle_root";
static PREFIX_CLAIM_INDEX: &[u8] = b"claim_index";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub anchor_token: CanonicalAddr,
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

pub fn store_latest_stage(storage: &mut dyn Storage, stage: u8) -> StdResult<()> {
    singleton(storage, KEY_LATEST_STAGE).save(&stage)
}

pub fn read_latest_stage(storage: &dyn Storage) -> StdResult<u8> {
    singleton_read(storage, KEY_LATEST_STAGE).load()
}

pub fn store_merkle_root(
    storage: &mut dyn Storage,
    stage: u8,
    merkle_root: String,
) -> StdResult<()> {
    let mut merkle_root_bucket: Bucket<String> = Bucket::new(storage, PREFIX_MERKLE_ROOT);
    merkle_root_bucket.save(&[stage], &merkle_root)
}

pub fn read_merkle_root(storage: &dyn Storage, stage: u8) -> StdResult<String> {
    let claim_index_bucket: ReadonlyBucket<String> =
        ReadonlyBucket::new(storage, PREFIX_MERKLE_ROOT);
    claim_index_bucket.load(&[stage])
}

pub fn store_claimed(storage: &mut dyn Storage, user: &CanonicalAddr, stage: u8) -> StdResult<()> {
    let mut claim_index_bucket: Bucket<bool> =
        Bucket::multilevel(storage, &[PREFIX_CLAIM_INDEX, user.as_slice()]);
    claim_index_bucket.save(&[stage], &true)
}

pub fn read_claimed(storage: &dyn Storage, user: &CanonicalAddr, stage: u8) -> StdResult<bool> {
    let claim_index_bucket: ReadonlyBucket<bool> =
        ReadonlyBucket::multilevel(storage, &[PREFIX_CLAIM_INDEX, user.as_slice()]);
    let res = claim_index_bucket.may_load(&[stage])?;
    match res {
        Some(v) => Ok(v),
        None => Ok(false),
    }
}
