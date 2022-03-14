#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CanonicalAddr, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};

use anchor_token::staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
    StakerInfoResponse, StateResponse,
};

use crate::{
    querier::query_anc_minter,
    state::{
        read_config, read_staker_info, read_state, remove_staker_info, store_config,
        store_staker_info, store_state, Config, StakerInfo, State,
    },
};

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use std::collections::BTreeMap;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    store_config(
        deps.storage,
        &Config {
            anchor_token: deps.api.addr_canonicalize(&msg.anchor_token)?,
            staking_token: deps.api.addr_canonicalize(&msg.staking_token)?,
            distribution_schedule: msg.distribution_schedule,
        },
    )?;

    store_state(
        deps.storage,
        &State {
            last_distributed: env.block.time.seconds(),
            total_bond_amount: Uint128::zero(),
            global_reward_index: Decimal::zero(),
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
        ExecuteMsg::Withdraw {} => withdraw(deps, env, info),
        ExecuteMsg::MigrateStaking {
            new_staking_contract,
        } => migrate_staking(deps, env, info, new_staking_contract),
        ExecuteMsg::UpdateConfig {
            distribution_schedule,
        } => update_config(deps, env, info, distribution_schedule),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => {
            // only staking token contract can execute this message
            if config.staking_token != deps.api.addr_canonicalize(info.sender.as_str())? {
                return Err(StdError::generic_err("unauthorized"));
            }

            let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;
            bond(deps, env, cw20_sender, cw20_msg.amount)
        }
        Err(_) => Err(StdError::generic_err("data should be given")),
    }
}

pub fn bond(deps: DepsMut, env: Env, sender_addr: Addr, amount: Uint128) -> StdResult<Response> {
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(sender_addr.as_str())?;

    let config: Config = read_config(deps.storage)?;
    let mut state: State = read_state(deps.storage)?;
    let mut staker_info: StakerInfo = read_staker_info(deps.storage, &sender_addr_raw)?;

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info)?;

    // Increase bond_amount
    increase_bond_amount(&mut state, &mut staker_info, amount);

    // Store updated state with staker's staker_info
    store_staker_info(deps.storage, &sender_addr_raw, &staker_info)?;
    store_state(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "bond"),
        ("owner", sender_addr.as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}

pub fn unbond(deps: DepsMut, env: Env, info: MessageInfo, amount: Uint128) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    let mut state: State = read_state(deps.storage)?;
    let mut staker_info: StakerInfo = read_staker_info(deps.storage, &sender_addr_raw)?;

    if staker_info.bond_amount < amount {
        return Err(StdError::generic_err("Cannot unbond more than bond amount"));
    }

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info)?;

    // Decrease bond_amount
    decrease_bond_amount(&mut state, &mut staker_info, amount)?;

    // Store or remove updated rewards info
    // depends on the left pending reward and bond amount
    if staker_info.pending_reward.is_zero() && staker_info.bond_amount.is_zero() {
        remove_staker_info(deps.storage, &sender_addr_raw);
    } else {
        store_staker_info(deps.storage, &sender_addr_raw, &staker_info)?;
    }

    // Store updated state
    store_state(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.staking_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            ("action", "unbond"),
            ("owner", info.sender.as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}

// withdraw rewards to executor
pub fn withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let sender_addr_raw = deps.api.addr_canonicalize(info.sender.as_str())?;

    let config: Config = read_config(deps.storage)?;
    let mut state: State = read_state(deps.storage)?;
    let mut staker_info = read_staker_info(deps.storage, &sender_addr_raw)?;

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info)?;

    let amount = staker_info.pending_reward;
    staker_info.pending_reward = Uint128::zero();

    // Store or remove updated rewards info
    // depends on the left pending reward and bond amount
    if staker_info.bond_amount.is_zero() {
        remove_staker_info(deps.storage, &sender_addr_raw);
    } else {
        store_staker_info(deps.storage, &sender_addr_raw, &staker_info)?;
    }

    // Store updated state
    store_state(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.anchor_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            ("action", "withdraw"),
            ("owner", info.sender.as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}

pub fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    distribution_schedule: Vec<(u64, u64, Uint128)>,
) -> StdResult<Response> {
    // get gov address by querying anc token minter
    let config: Config = read_config(deps.storage)?;
    let state: State = read_state(deps.storage)?;

    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    let anc_token: Addr = deps.api.addr_humanize(&config.anchor_token)?;
    let gov_addr_raw: CanonicalAddr = deps
        .api
        .addr_canonicalize(&query_anc_minter(&deps.querier, anc_token)?)?;
    if sender_addr_raw != gov_addr_raw {
        return Err(StdError::generic_err("unauthorized"));
    }

    assert_new_schedules(&config, &state, distribution_schedule.clone())?;

    let new_config = Config {
        anchor_token: config.anchor_token,
        staking_token: config.staking_token,
        distribution_schedule,
    };
    store_config(deps.storage, &new_config)?;

    Ok(Response::new().add_attributes(vec![("action", "update_config")]))
}

pub fn migrate_staking(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_staking_contract: String,
) -> StdResult<Response> {
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    let mut config: Config = read_config(deps.storage)?;
    let mut state: State = read_state(deps.storage)?;
    let anc_token: Addr = deps.api.addr_humanize(&config.anchor_token)?;

    // get gov address by querying anc token minter
    let gov_addr_raw: CanonicalAddr = deps
        .api
        .addr_canonicalize(&query_anc_minter(&deps.querier, anc_token.clone())?)?;
    if sender_addr_raw != gov_addr_raw {
        return Err(StdError::generic_err("unauthorized"));
    }

    // compute global reward, sets last_distributed_seconds to env.block.time.seconds
    compute_reward(&config, &mut state, env.block.time.seconds());

    let total_distribution_amount: Uint128 =
        config.distribution_schedule.iter().map(|item| item.2).sum();

    let block_time = env.block.time.seconds();
    // eliminate distribution slots that have not started
    config
        .distribution_schedule
        .retain(|slot| slot.0 < block_time);

    let mut distributed_amount = Uint128::zero();
    for s in config.distribution_schedule.iter_mut() {
        if s.1 < block_time {
            // all distributed
            distributed_amount += s.2;
        } else {
            // partially distributed slot
            let whole_time = s.1 - s.0;
            let distribution_amount_per_second: Decimal = Decimal::from_ratio(s.2, whole_time);

            let passed_time = block_time - s.0;
            let distributed_amount_on_slot =
                distribution_amount_per_second * Uint128::from(passed_time as u128);
            distributed_amount += distributed_amount_on_slot;

            // modify distribution slot
            s.1 = block_time;
            s.2 = distributed_amount_on_slot;
        }
    }

    // update config
    store_config(deps.storage, &config)?;
    // update state
    store_state(deps.storage, &state)?;

    let remaining_anc = total_distribution_amount.checked_sub(distributed_amount)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: anc_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: new_staking_contract,
                amount: remaining_anc,
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            ("action", "migrate_staking"),
            ("distributed_amount", &distributed_amount.to_string()),
            ("remaining_amount", &remaining_anc.to_string()),
        ]))
}

fn increase_bond_amount(state: &mut State, staker_info: &mut StakerInfo, amount: Uint128) {
    state.total_bond_amount += amount;
    staker_info.bond_amount += amount;
}

fn decrease_bond_amount(
    state: &mut State,
    staker_info: &mut StakerInfo,
    amount: Uint128,
) -> StdResult<()> {
    state.total_bond_amount = state.total_bond_amount.checked_sub(amount)?;
    staker_info.bond_amount = staker_info.bond_amount.checked_sub(amount)?;
    Ok(())
}

// compute distributed rewards and update global reward index
fn compute_reward(config: &Config, state: &mut State, block_time: u64) {
    if state.total_bond_amount.is_zero() {
        state.last_distributed = block_time;
        return;
    }

    let mut distributed_amount: Uint128 = Uint128::zero();
    for s in config.distribution_schedule.iter() {
        if s.0 > block_time || s.1 < state.last_distributed {
            continue;
        }

        // min(s.1, block_time) - max(s.0, last_distributed)
        let passed_time =
            std::cmp::min(s.1, block_time) - std::cmp::max(s.0, state.last_distributed);

        let time = s.1 - s.0;
        let distribution_amount_per_second: Decimal = Decimal::from_ratio(s.2, time);
        distributed_amount += distribution_amount_per_second * Uint128::from(passed_time as u128);
    }

    state.last_distributed = block_time;
    state.global_reward_index = state.global_reward_index
        + Decimal::from_ratio(distributed_amount, state.total_bond_amount);
}

// withdraw reward to pending reward
fn compute_staker_reward(state: &State, staker_info: &mut StakerInfo) -> StdResult<()> {
    let pending_reward = (staker_info.bond_amount * state.global_reward_index)
        .checked_sub(staker_info.bond_amount * staker_info.reward_index)?;

    staker_info.reward_index = state.global_reward_index;
    staker_info.pending_reward += pending_reward;
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State { block_time } => to_binary(&query_state(deps, block_time)?),
        QueryMsg::StakerInfo { staker, block_time } => {
            to_binary(&query_staker_info(deps, staker, block_time)?)
        }
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        anchor_token: deps.api.addr_humanize(&state.anchor_token)?.to_string(),
        staking_token: deps.api.addr_humanize(&state.staking_token)?.to_string(),
        distribution_schedule: state.distribution_schedule,
    };

    Ok(resp)
}

pub fn query_state(deps: Deps, block_time: Option<u64>) -> StdResult<StateResponse> {
    let mut state: State = read_state(deps.storage)?;
    if let Some(block_time) = block_time {
        let config = read_config(deps.storage)?;
        compute_reward(&config, &mut state, block_time);
    }

    Ok(StateResponse {
        last_distributed: state.last_distributed,
        total_bond_amount: state.total_bond_amount,
        global_reward_index: state.global_reward_index,
    })
}

pub fn query_staker_info(
    deps: Deps,
    staker: String,
    block_time: Option<u64>,
) -> StdResult<StakerInfoResponse> {
    let staker_raw = deps.api.addr_canonicalize(&staker)?;

    let mut staker_info: StakerInfo = read_staker_info(deps.storage, &staker_raw)?;
    if let Some(block_time) = block_time {
        let config = read_config(deps.storage)?;
        let mut state = read_state(deps.storage)?;

        compute_reward(&config, &mut state, block_time);
        compute_staker_reward(&state, &mut staker_info)?;
    }

    Ok(StakerInfoResponse {
        staker,
        reward_index: staker_info.reward_index,
        bond_amount: staker_info.bond_amount,
        pending_reward: staker_info.pending_reward,
    })
}

pub fn assert_new_schedules(
    config: &Config,
    state: &State,
    distribution_schedule: Vec<(u64, u64, Uint128)>,
) -> StdResult<()> {
    if distribution_schedule.len() < config.distribution_schedule.len() {
        return Err(StdError::generic_err(
            "cannot update; the new schedule must support all of the previous schedule",
        ));
    }

    let mut existing_counts: BTreeMap<(u64, u64, Uint128), u32> = BTreeMap::new();
    for schedule in config.distribution_schedule.clone() {
        let counter = existing_counts.entry(schedule).or_insert(0);
        *counter += 1;
    }

    let mut new_counts: BTreeMap<(u64, u64, Uint128), u32> = BTreeMap::new();
    for schedule in distribution_schedule {
        let counter = new_counts.entry(schedule).or_insert(0);
        *counter += 1;
    }

    for (schedule, count) in existing_counts.into_iter() {
        // if began ensure its in the new schedule
        if schedule.0 <= state.last_distributed {
            if count > *new_counts.get(&schedule).unwrap_or(&0u32) {
                return Err(StdError::generic_err(
                    "new schedule removes already started distribution",
                ));
            }
            // after this new_counts will only contain the newly added schedules
            *new_counts.get_mut(&schedule).unwrap() -= count;
        }
    }

    for (schedule, count) in new_counts.into_iter() {
        if count > 0 && schedule.0 <= state.last_distributed {
            return Err(StdError::generic_err(
                "new schedule adds an already started distribution",
            ));
        }
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
