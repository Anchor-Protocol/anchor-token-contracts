use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{store_config, Config, KEY_CONFIG};
use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::ReadonlySingleton;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
struct LegacyConfig {
    pub owner: CanonicalAddr,
    pub token_contract: CanonicalAddr,
}

fn read_legacy_config(storage: &dyn Storage) -> StdResult<LegacyConfig> {
    ReadonlySingleton::new(storage, KEY_CONFIG).load()
}

pub fn migrate_config(storage: &mut dyn Storage, gov_contract: CanonicalAddr) -> StdResult<()> {
    let legacy_config: LegacyConfig = read_legacy_config(storage)?;

    store_config(
        storage,
        &Config {
            owner: legacy_config.owner,
            anchor_token: legacy_config.token_contract,
            gov_contract,
        },
    )
}
