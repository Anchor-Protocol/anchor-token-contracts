use crate::error::ContractError;
use crate::state::{
    bank_read, bank_store, config_read, config_store, is_synced_read, is_synced_store, poll_read,
    poll_store, poll_voter_read, poll_voter_store, read_polls, state_read, state_store, Config,
    Poll, State, TokenManager,
};
use crate::voting_escrow::{
    generate_extend_lock_amount_message, generate_extend_lock_time_message,
    generate_withdraw_message,
};

use anchor_token::gov::{PollStatus, StakerResponse, VoterInfo};
use astroport::querier::query_token_balance;
use cosmwasm_std::{
    to_binary, CanonicalAddr, CosmosMsg, Deps, DepsMut, MessageInfo, Response, StdResult, Storage,
    Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

// can only be called when user's unlock_time > current_time
pub fn extend_lock_amount(
    deps: DepsMut,
    sender: CanonicalAddr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount.is_zero() {
        return Err(ContractError::InsufficientFunds {});
    }

    let key = &sender.as_slice();

    let mut token_manager = bank_read(deps.storage).may_load(key)?.unwrap_or_default();
    let config: Config = config_store(deps.storage).load()?;
    let mut state: State = state_store(deps.storage).load()?;

    let total_locked_balance = state.total_deposit + state.pending_voting_rewards;

    // balance already increased, so subtract deposit amount
    let total_balance = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.anchor_token)?,
        deps.api.addr_humanize(&state.contract_addr)?,
    )?
    .checked_sub(total_locked_balance + amount)?;

    let share = if total_balance.is_zero() || state.total_share.is_zero() {
        amount
    } else {
        amount.multiply_ratio(state.total_share, total_balance)
    };

    token_manager.share += share;
    state.total_share += share;

    state_store(deps.storage).save(&state)?;
    bank_store(deps.storage).save(key, &token_manager)?;

    let extend_lock_amount_message = generate_extend_lock_amount_message(
        deps.as_ref(),
        &config.anchor_voting_escrow,
        &sender,
        share,
    )?;

    Ok(Response::new()
        .add_message(extend_lock_amount_message)
        .add_attributes(vec![
            ("action", "extend_lock_amount"),
            (
                "sender",
                deps.api.addr_humanize(&sender)?.to_string().as_str(),
            ),
            ("share", share.to_string().as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}

// can be called anytime.
pub fn extend_lock_time(
    deps: DepsMut,
    sender: CanonicalAddr,
    time: u64,
) -> Result<Response, ContractError> {
    let config: Config = config_store(deps.storage).load()?;
    let key = &sender.as_slice();

    let mut messages: Vec<CosmosMsg> = vec![generate_extend_lock_time_message(
        deps.as_ref(),
        &config.anchor_voting_escrow,
        &sender,
        time,
    )?];

    let is_synced = is_synced_read(deps.storage)
        .may_load(key)?
        .unwrap_or_default();

    if !is_synced {
        let token_manager = bank_read(deps.storage).may_load(key)?.unwrap_or_default();

        if !token_manager.share.is_zero() {
            messages.push(generate_extend_lock_amount_message(
                deps.as_ref(),
                &config.anchor_voting_escrow,
                &sender,
                token_manager.share,
            )?);
        }

        is_synced_store(deps.storage).save(key, &true)?;
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "extend_lock_time"),
        ("sender", &deps.api.addr_humanize(&sender)?.to_string()),
        ("time", &time.to_string()),
    ]))
}

pub fn deposit_reward(deps: DepsMut, amount: Uint128) -> Result<Response, ContractError> {
    let config = config_read(deps.storage).load()?;

    let mut polls_in_progress = read_polls(
        deps.storage,
        Some(PollStatus::InProgress),
        None,
        None,
        None,
        Some(true), // remove hard cap to get all polls
    )?;

    if config.voter_weight.is_zero() || polls_in_progress.is_empty() {
        return Ok(Response::new().add_attributes(vec![
            ("action", "deposit_reward"),
            ("amount", &amount.to_string()),
        ]));
    }

    let voter_rewards = amount * config.voter_weight;
    let rewards_per_poll =
        voter_rewards.multiply_ratio(Uint128::new(1), polls_in_progress.len() as u128);
    if rewards_per_poll.is_zero() {
        return Err(ContractError::RewardDepositedTooSmall {});
    }
    for poll in polls_in_progress.iter_mut() {
        poll.voters_reward += rewards_per_poll;
        poll_store(deps.storage)
            .save(&poll.id.to_be_bytes(), poll)
            .unwrap()
    }

    state_store(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.pending_voting_rewards += voter_rewards;
        Ok(state)
    })?;

    Ok(Response::new().add_attributes(vec![
        ("action", "deposit_reward"),
        ("amount", &amount.to_string()),
    ]))
}

fn get_withdrawable_polls(
    storage: &dyn Storage,
    token_manager: &TokenManager,
    user_address: &CanonicalAddr,
) -> Vec<(Poll, VoterInfo)> {
    let w_polls: Vec<(Poll, VoterInfo)> = token_manager
        .locked_balance
        .iter()
        .map(|(poll_id, _)| {
            let poll: Poll = poll_read(storage).load(&poll_id.to_be_bytes()).unwrap();
            let voter_info_res: StdResult<VoterInfo> =
                poll_voter_read(storage, *poll_id).load(user_address.as_slice());
            (poll, voter_info_res)
        })
        .filter(|(poll, voter_info_res)| {
            poll.status != PollStatus::InProgress
                && voter_info_res.is_ok()
                && !poll.voters_reward.is_zero()
        })
        .map(|(poll, voter_info_res)| (poll, voter_info_res.unwrap()))
        .collect();
    w_polls
}

fn withdraw_user_voting_rewards(
    storage: &mut dyn Storage,
    user_address: &CanonicalAddr,
    token_manager: &TokenManager,
    poll_id: Option<u64>,
) -> Result<(u128, Vec<u64>), ContractError> {
    let w_polls: Vec<(Poll, VoterInfo)> = match poll_id {
        Some(poll_id) => {
            let poll: Poll = poll_read(storage).load(&poll_id.to_be_bytes())?;
            let voter_info = poll_voter_read(storage, poll_id).load(user_address.as_slice())?;
            if poll.status == PollStatus::InProgress {
                return Err(ContractError::PollInProgress {});
            }
            if poll.voters_reward.is_zero() {
                return Err(ContractError::InsufficientReward {});
            }
            vec![(poll, voter_info)]
        }
        None => get_withdrawable_polls(storage, token_manager, user_address),
    };
    let user_reward_amount: u128 = w_polls
        .iter()
        .map(|(poll, voting_info)| {
            // remove voter info from the poll
            poll_voter_store(storage, poll.id).remove(user_address.as_slice());

            // calculate reward share
            let total_votes = poll.no_votes.u128() + poll.yes_votes.u128();
            let poll_voting_reward = poll
                .voters_reward
                .multiply_ratio(voting_info.balance, total_votes);
            poll_voting_reward.u128()
        })
        .sum();
    Ok((
        user_reward_amount,
        w_polls.iter().map(|(poll, _)| poll.id).collect(),
    ))
}

pub fn withdraw_voting_rewards(
    deps: DepsMut,
    info: MessageInfo,
    poll_id: Option<u64>,
) -> Result<Response, ContractError> {
    let config: Config = config_store(deps.storage).load()?;
    let sender_address_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let key = sender_address_raw.as_slice();

    let mut token_manager = bank_read(deps.storage)
        .load(key)
        .map_err(|_| ContractError::NothingStaked {})?;

    let (user_reward_amount, w_polls) =
        withdraw_user_voting_rewards(deps.storage, &sender_address_raw, &token_manager, poll_id)?;
    if user_reward_amount.eq(&0u128) {
        return Err(ContractError::NothingToWithdraw {});
    }

    // cleanup, remove from locked_balance the polls from which we withdrew the rewards
    token_manager
        .locked_balance
        .retain(|(poll_id, _)| !w_polls.contains(poll_id));
    bank_store(deps.storage).save(key, &token_manager)?;

    state_store(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.pending_voting_rewards = state
            .pending_voting_rewards
            .checked_sub(Uint128::new(user_reward_amount))?;
        Ok(state)
    })?;

    send_tokens(
        deps,
        &config.anchor_token,
        &sender_address_raw,
        user_reward_amount,
        "withdraw_voting_rewards",
    )
}

// Withdraw amount if not staked. By default all funds will be withdrawn.
pub fn withdraw_voting_tokens(
    deps: DepsMut,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let sender_address_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let key = sender_address_raw.as_slice();

    if let Some(mut token_manager) = bank_read(deps.storage).may_load(key)? {
        let config: Config = config_store(deps.storage).load()?;
        let mut state: State = state_store(deps.storage).load()?;

        let total_locked_balance = state.total_deposit + state.pending_voting_rewards;

        // Load total share & total balance except proposal deposit amount
        let total_share = state.total_share.u128();
        let total_balance = query_token_balance(
            &deps.querier,
            deps.api.addr_humanize(&config.anchor_token)?,
            deps.api.addr_humanize(&state.contract_addr)?,
        )?
        .checked_sub(total_locked_balance)?
        .u128();

        token_manager.locked_balance.retain(|(poll_id, _)| {
            let poll: Poll = poll_read(deps.storage)
                .load(&poll_id.to_be_bytes())
                .unwrap();

            if poll.status != PollStatus::InProgress && poll.voters_reward.is_zero() {
                // remove voter info from the poll
                poll_voter_store(deps.storage, *poll_id).remove(sender_address_raw.as_slice());
            }

            poll.status == PollStatus::InProgress || !poll.voters_reward.is_zero()
        });

        let user_share = token_manager.share.u128();

        let withdraw_share = amount
            .map(|v| std::cmp::max(v.multiply_ratio(total_share, total_balance).u128(), 1u128))
            .unwrap_or_else(|| user_share);
        let withdraw_amount = amount
            .map(|v| v.u128())
            .unwrap_or_else(|| withdraw_share * total_balance / total_share);

        if withdraw_share > user_share {
            Err(ContractError::InvalidWithdrawAmount {})
        } else {
            let share = user_share - withdraw_share;
            token_manager.share = Uint128::from(share);

            bank_store(deps.storage).save(key, &token_manager)?;

            state.total_share = Uint128::from(total_share - withdraw_share);
            state_store(deps.storage).save(&state)?;

            let is_synced = is_synced_read(deps.storage)
                .may_load(key)?
                .unwrap_or_default();

            let mut messages: Vec<CosmosMsg> = vec![];

            if is_synced {
                messages.push(generate_withdraw_message(
                    deps.as_ref(),
                    &config.anchor_voting_escrow,
                    &sender_address_raw,
                    Uint128::from(withdraw_share),
                )?);
            }

            Ok(send_tokens(
                deps,
                &config.anchor_token,
                &sender_address_raw,
                withdraw_amount,
                "withdraw",
            )?
            .add_messages(messages))
        }
    } else {
        Err(ContractError::NothingStaked {})
    }
}

fn send_tokens(
    deps: DepsMut,
    asset_token: &CanonicalAddr,
    recipient: &CanonicalAddr,
    amount: u128,
    action: &str,
) -> Result<Response, ContractError> {
    let contract_human = deps.api.addr_humanize(asset_token)?.to_string();
    let recipient_human = deps.api.addr_humanize(recipient)?.to_string();

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_human,
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient_human.clone(),
                amount: Uint128::from(amount),
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            ("action", action),
            ("recipient", recipient_human.as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}

pub fn query_staker(deps: Deps, address: String) -> StdResult<StakerResponse> {
    let addr_raw = deps.api.addr_canonicalize(&address).unwrap();
    let config: Config = config_read(deps.storage).load()?;
    let state: State = state_read(deps.storage).load()?;
    let mut token_manager = bank_read(deps.storage)
        .may_load(addr_raw.as_slice())?
        .unwrap_or_default();

    // calculate pending voting rewards
    let w_polls: Vec<(Poll, VoterInfo)> =
        get_withdrawable_polls(deps.storage, &token_manager, &addr_raw);

    let mut user_reward_amount = Uint128::zero();
    let w_polls_res: Vec<(u64, Uint128)> = w_polls
        .iter()
        .map(|(poll, voting_info)| {
            // calculate reward share
            let total_votes = poll.no_votes + poll.yes_votes;
            let poll_voting_reward = poll
                .voters_reward
                .multiply_ratio(voting_info.balance, total_votes);
            user_reward_amount += poll_voting_reward;

            (poll.id, poll_voting_reward)
        })
        .collect();

    // filter out not in-progress polls
    token_manager.locked_balance.retain(|(poll_id, _)| {
        let poll: Poll = poll_read(deps.storage)
            .load(&poll_id.to_be_bytes())
            .unwrap();

        poll.status == PollStatus::InProgress
    });

    let total_locked_balance = state.total_deposit + state.pending_voting_rewards;
    let total_balance = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.anchor_token)?,
        deps.api.addr_humanize(&state.contract_addr)?,
    )?
    .checked_sub(total_locked_balance)?;

    Ok(StakerResponse {
        balance: if !state.total_share.is_zero() {
            token_manager
                .share
                .multiply_ratio(total_balance, state.total_share)
        } else {
            Uint128::zero()
        },
        share: token_manager.share,
        locked_balance: token_manager.locked_balance,
        pending_voting_rewards: user_reward_amount,
        withdrawable_polls: w_polls_res,
    })
}
