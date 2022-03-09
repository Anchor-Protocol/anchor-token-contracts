use crate::state::{config_store, legacy_config_read, Config, LegacyConfig};
use cosmwasm_std::{CanonicalAddr, StdResult, Storage};

pub fn migrate_config(
    storage: &mut dyn Storage,
    anchor_voting_escrow: CanonicalAddr,
) -> StdResult<()> {
    let legacy_config: LegacyConfig = legacy_config_read(storage)?;

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
