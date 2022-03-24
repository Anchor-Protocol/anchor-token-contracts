use crate::error::ContractError;
use crate::state::{
    bank_read, bank_store, config_read, config_store, is_synced_read, is_synced_store, poll_read,
    poll_voter_store, state_read, state_store, Config, Poll, State,
};
use crate::voting_escrow::{
    generate_extend_lock_amount_message, generate_extend_lock_time_message,
    generate_withdraw_message,
};

use anchor_token::gov::{PollStatus, StakerResponse};
use astroport::querier::query_token_balance;
use cosmwasm_std::{
    to_binary, CanonicalAddr, CosmosMsg, Deps, DepsMut, MessageInfo, Response, StdResult, Uint128,
    WasmMsg,
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

    // balance already increased, so subtract deposit amount
    let total_balance = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.anchor_token)?,
        deps.api.addr_humanize(&state.contract_addr)?,
    )?
    .checked_sub(state.total_deposit + amount)?;

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

        messages.push(generate_extend_lock_amount_message(
            deps.as_ref(),
            &config.anchor_voting_escrow,
            &sender,
            token_manager.share,
        )?);
        is_synced_store(deps.storage).save(key, &true)?;
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "extend_lock_time"),
        ("sender", sender.to_string().as_str()),
        ("time", time.to_string().as_str()),
    ]))
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

        // Load total share & total balance except proposal deposit amount
        let total_share = state.total_share.u128();
        let total_balance = query_token_balance(
            &deps.querier,
            deps.api.addr_humanize(&config.anchor_token)?,
            deps.api.addr_humanize(&state.contract_addr)?,
        )?
        .checked_sub(state.total_deposit)?
        .u128();

        token_manager.locked_balance.retain(|(poll_id, _)| {
            let poll: Poll = poll_read(deps.storage)
                .load(&poll_id.to_be_bytes())
                .unwrap();

            if poll.status != PollStatus::InProgress {
                // remove voter info from the poll
                poll_voter_store(deps.storage, *poll_id).remove(sender_address_raw.as_slice());
            }

            poll.status == PollStatus::InProgress
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

    // filter out not in-progress polls
    token_manager.locked_balance.retain(|(poll_id, _)| {
        let poll: Poll = poll_read(deps.storage)
            .load(&poll_id.to_be_bytes())
            .unwrap();

        poll.status == PollStatus::InProgress
    });

    let total_balance = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.anchor_token)?,
        deps.api.addr_humanize(&state.contract_addr)?,
    )?
    .checked_sub(state.total_deposit)?;

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
    })
}
