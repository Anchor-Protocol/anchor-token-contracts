use cosmwasm_std::{CanonicalAddr, Order, StdResult, Storage};
use cw_storage_plus::{Bound, Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use anchor_token::common::OrderBy;
use anchor_token::vesting::VestingInfo;

const CONFIG: Item<Config> = Item::new("config");
const VESTING_INFO: Map<&[u8], VestingInfo> = Map::new("vesting_info");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub anchor_token: CanonicalAddr,
    pub genesis_time: u64,
    pub last_claim_deadline: u64,
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    CONFIG.save(storage, config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    CONFIG.load(storage)
}

pub fn read_vesting_info(storage: &dyn Storage, address: &CanonicalAddr) -> StdResult<VestingInfo> {
    VESTING_INFO.load(storage, address.as_slice())
}

pub fn store_vesting_info(
    storage: &mut dyn Storage,
    address: &CanonicalAddr,
    vesting_info: &VestingInfo,
) -> StdResult<()> {
    VESTING_INFO.save(storage, address.as_slice(), vesting_info)
}

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn read_vesting_infos(
    storage: &dyn Storage,
    start_after: Option<CanonicalAddr>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<(CanonicalAddr, VestingInfo)>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order) = match order_by {
        Some(OrderBy::Desc) => (
            None,
            calc_range_end_addr(start_after).map(Bound::exclusive),
            Order::Descending,
        ),
        _ => (
            calc_range_start_addr(start_after).map(Bound::inclusive),
            None,
            Order::Ascending,
        ),
    };

    VESTING_INFO
        .range(storage, start, end, order)
        .take(limit)
        .map(|item| {
            let (k, v) = item?;
            Ok((CanonicalAddr::from(k), v))
        })
        .collect()
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start_addr(start_after: Option<CanonicalAddr>) -> Option<Vec<u8>> {
    start_after.map(|addr| {
        let mut v = addr.as_slice().to_vec();
        v.push(1);
        v
    })
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_end_addr(start_after: Option<CanonicalAddr>) -> Option<Vec<u8>> {
    start_after.map(|addr| addr.as_slice().to_vec())
}
