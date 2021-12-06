use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{store_config, Config, KEY_CONFIG};
use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::ReadonlySingleton;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub gov_contract: CanonicalAddr,      // collected rewards receiver
    pub terraswap_factory: CanonicalAddr, // terraswap factory contract
    pub anchor_token: CanonicalAddr,      // anchor token address
    pub reward_factor: Decimal256, // reward distribution rate to gov contract, left rewards sent back to distributor contract
}

fn read_legacy_config(storage: &dyn Storage) -> StdResult<LegacyConfig> {
    ReadonlySingleton::new(storage, KEY_CONFIG).load()
}

pub fn migrate_config(storage: &mut dyn Storage) -> StdResult<()> {
    let legacy_config: LegacyConfig = read_legacy_config(storage)?;

    store_config(
        storage,
        &Config {
            gov_contract: legacy_config.gov_contract,
            terraswap_factory: legacy_config.terraswap_factory,
            anchor_token: legacy_config.anchor_token,
            reward_factor: legacy_config.reward_factor,
        },
    )
}
