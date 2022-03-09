use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{config_store, Config};
use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage, Uint128};
use cosmwasm_storage::ReadonlySingleton;

static KEY_LEGACY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub owner: CanonicalAddr,
    pub anchor_token: CanonicalAddr,
    pub quorum: Decimal,
    pub threshold: Decimal,
    pub voting_period: u64,
    pub timelock_period: u64,
    pub expiration_period: u64,
    pub proposal_deposit: Uint128,
    pub snapshot_period: u64,
}

fn read_legacy_config(storage: &dyn Storage) -> StdResult<LegacyConfig> {
    ReadonlySingleton::new(storage, KEY_LEGACY_CONFIG).load()
}

pub fn migrate_config(
    storage: &mut dyn Storage,
    anchor_voting_escrow: CanonicalAddr,
) -> StdResult<()> {
    let legacy_config: LegacyConfig = read_legacy_config(storage)?;

    config_store(storage).save(&Config {
        owner: legacy_config.owner,
        anchor_token: legacy_config.anchor_token,
        quorum: legacy_config.quorum,
        threshold: legacy_config.threshold,
        voting_period: legacy_config.voting_period,
        timelock_period: legacy_config.timelock_period,
        expiration_period: legacy_config.expiration_period,
        proposal_deposit: legacy_config.proposal_deposit,
        snapshot_period: legacy_config.snapshot_period,
        anchor_voting_escrow,
    })
}
