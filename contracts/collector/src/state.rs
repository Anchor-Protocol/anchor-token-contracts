use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read};

static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub gov_contract: CanonicalAddr,      // collected rewards receiver
    pub terraswap_factory: CanonicalAddr, // terraswap factory contract
    pub anchor_token: CanonicalAddr,      // anchor token address
    pub distributor_contract: CanonicalAddr,   // distributor contract to sent back rewards
    pub reward_factor: Decimal, // reward distribution rate to gov contract, left rewards sent back to distributor contract
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}
