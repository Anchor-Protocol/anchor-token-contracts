use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read};

pub static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub gov_contract: CanonicalAddr,      // collected rewards receiver
    pub terraswap_factory: CanonicalAddr, // terraswap factory contract
    pub anchor_token: CanonicalAddr,      // anchor token address
    pub reward_factor: Decimal256, // reward distribution rate to gov contract, left rewards sent back to distributor contract
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}
