use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{config_store, poll_store, state_store, Config, ExecuteData, Poll, State};
use anchor_token::gov::PollStatus;
use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket_read, singleton_read};

pub static KEY_LEGACY_CONFIG: &[u8] = b"config";
pub static KEY_LEGACY_STATE: &[u8] = b"state";
pub static PREFIX_LEGACY_POLL: &[u8] = b"poll";

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyState {
    pub contract_addr: CanonicalAddr,
    pub poll_count: u64,
    pub total_share: Uint128,
    pub total_deposit: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyPoll {
    pub id: u64,
    pub creator: CanonicalAddr,
    pub status: PollStatus,
    pub yes_votes: Uint128,
    pub no_votes: Uint128,
    pub end_height: u64,
    pub title: String,
    pub description: String,
    pub link: Option<String>,
    pub execute_data: Option<Vec<ExecuteData>>,
    pub deposit_amount: Uint128,
    /// Total balance at the end poll
    pub total_balance_at_end_poll: Option<Uint128>,
    pub staked_amount: Option<Uint128>,
}

pub fn read_legacy_config(storage: &dyn Storage) -> StdResult<LegacyConfig> {
    singleton_read(storage, KEY_LEGACY_CONFIG).load()
}

pub fn read_legacy_state(storage: &dyn Storage) -> StdResult<LegacyState> {
    singleton_read(storage, KEY_LEGACY_STATE).load()
}

pub fn read_legacy_poll(storage: &dyn Storage, poll_id: u64) -> StdResult<LegacyPoll> {
    bucket_read(storage, PREFIX_LEGACY_POLL).load(&poll_id.to_be_bytes())
}

pub fn migrate_config(
    storage: &mut dyn Storage,
    anchor_voting_escrow: CanonicalAddr,
    voter_weight: Decimal,
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
        voter_weight,
    })?;

    Ok(())
}

pub fn migrate_state(storage: &mut dyn Storage) -> StdResult<()> {
    let legacy_state: LegacyState = read_legacy_state(storage)?;

    state_store(storage).save(&State {
        contract_addr: legacy_state.contract_addr,
        poll_count: legacy_state.poll_count,
        total_share: legacy_state.total_share,
        total_deposit: legacy_state.total_deposit,
        pending_voting_rewards: Uint128::zero(),
    })?;

    Ok(())
}

pub fn migrate_polls(storage: &mut dyn Storage, poll_count: u64) -> StdResult<()> {
    for poll_id in 1..=poll_count {
        let legacy_poll: LegacyPoll = read_legacy_poll(storage, poll_id)?;

        poll_store(storage).save(
            &poll_id.to_be_bytes(),
            &Poll {
                id: legacy_poll.id,
                creator: legacy_poll.creator,
                status: legacy_poll.status,
                yes_votes: legacy_poll.yes_votes,
                no_votes: legacy_poll.no_votes,
                end_height: legacy_poll.end_height,
                title: legacy_poll.title,
                description: legacy_poll.description,
                link: legacy_poll.link,
                execute_data: legacy_poll.execute_data,
                deposit_amount: legacy_poll.deposit_amount,
                total_balance_at_end_poll: legacy_poll.total_balance_at_end_poll,
                staked_amount: legacy_poll.staked_amount,
                voters_reward: Uint128::zero(),
            },
        )?;
    }

    Ok(())
}
