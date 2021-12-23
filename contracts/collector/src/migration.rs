use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{store_config, Config, KEY_CONFIG};
use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage};
use cosmwasm_storage::ReadonlySingleton;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub gov_contract: CanonicalAddr,         // collected rewards receiver
    pub terraswap_factory: CanonicalAddr,    // astroport factory contract
    pub anchor_token: CanonicalAddr,         // anchor token address
    pub distributor_contract: CanonicalAddr, // distributor contract to sent back rewards
    pub reward_factor: Decimal, // reward distribution rate to gov contract, left rewards sent back to distributor contract
}

fn read_legacy_config(storage: &dyn Storage) -> StdResult<LegacyConfig> {
    ReadonlySingleton::new(storage, KEY_CONFIG).load()
}

pub fn migrate_config(storage: &mut dyn Storage, astroport_factory: CanonicalAddr) -> StdResult<()> {
    let legacy_config: LegacyConfig = read_legacy_config(storage)?;

    store_config(
        storage,
        &Config {
            gov_contract: legacy_config.gov_contract,
            astroport_factory,
            anchor_token: legacy_config.anchor_token,
            distributor_contract: legacy_config.distributor_contract,
            reward_factor: legacy_config.reward_factor,
        },
    )
}
