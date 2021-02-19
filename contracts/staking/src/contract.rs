use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, MigrateResponse, MigrateResult, Querier,
    StdError, StdResult, Storage, Uint128, WasmMsg,
};

use anchor_token::staking::{
    ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, MigrateMsg, QueryMsg, StakerInfoResponse,
    StateResponse,
};

use crate::state::{
    read_config, read_staker_info, read_state, remove_staker_info, store_config, store_staker_info,
    store_state, Config, StakerInfo, State,
};

use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            anchor_token: deps.api.canonical_address(&msg.anchor_token)?,
            staking_token: deps.api.canonical_address(&msg.staking_token)?,
            distribution_schedule: msg.distribution_schedule,
        },
    )?;

    store_state(
        &mut deps.storage,
        &State {
            last_distributed: env.block.height,
            total_bond_amount: Uint128::zero(),
            global_reward_index: Decimal::zero(),
        },
    )?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive(msg) => receive_cw20(deps, env, msg),
        HandleMsg::Unbond { amount } => unbond(deps, env, amount),
        HandleMsg::Withdraw {} => withdraw(deps, env),
    }
}

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    if let Some(msg) = cw20_msg.msg {
        let config: Config = read_config(&deps.storage)?;

        match from_binary(&msg)? {
            Cw20HookMsg::Bond {} => {
                // only staking token contract can execute this message
                if config.staking_token != deps.api.canonical_address(&env.message.sender)? {
                    return Err(StdError::unauthorized());
                }

                bond(deps, env, cw20_msg.sender, cw20_msg.amount)
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn bond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender_addr: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let sender_addr_raw: CanonicalAddr = deps.api.canonical_address(&sender_addr)?;

    let config: Config = read_config(&deps.storage)?;
    let mut state: State = read_state(&deps.storage)?;
    let mut staker_info: StakerInfo = read_staker_info(&deps.storage, &sender_addr_raw)?;

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.height);
    compute_staker_reward(&state, &mut staker_info)?;

    // Increase bond_amount
    increase_bond_amount(&mut state, &mut staker_info, amount);

    // Store updated state with staker's staker_info
    store_staker_info(&mut deps.storage, &sender_addr_raw, &staker_info)?;
    store_state(&mut deps.storage, &state)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "bond"),
            log("owner", sender_addr),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

pub fn unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;

    let mut state: State = read_state(&deps.storage)?;
    let mut staker_info: StakerInfo = read_staker_info(&deps.storage, &sender_addr_raw)?;

    if staker_info.bond_amount < amount {
        return Err(StdError::generic_err("Cannot unbond more than bond amount"));
    }

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.height);
    compute_staker_reward(&state, &mut staker_info)?;

    // Decrease bond_amount
    decrease_bond_amount(&mut state, &mut staker_info, amount)?;

    // Store or remove updated rewards info
    // depends on the left pending reward and bond amount
    if staker_info.pending_reward.is_zero() && staker_info.bond_amount.is_zero() {
        remove_staker_info(&mut deps.storage, &sender_addr_raw);
    } else {
        store_staker_info(&mut deps.storage, &sender_addr_raw, &staker_info)?;
    }

    // Store updated state
    store_state(&mut deps.storage, &state)?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.staking_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: env.message.sender.clone(),
                amount,
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "unbond"),
            log("owner", env.message.sender),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

// withdraw rewards to executor
pub fn withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let sender_addr_raw = deps.api.canonical_address(&env.message.sender)?;

    let config: Config = read_config(&deps.storage)?;
    let mut state: State = read_state(&deps.storage)?;
    let mut staker_info = read_staker_info(&deps.storage, &sender_addr_raw)?;

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.height);
    compute_staker_reward(&state, &mut staker_info)?;

    let amount = staker_info.pending_reward;
    staker_info.pending_reward = Uint128::zero();

    // Store or remove updated rewards info
    // depends on the left pending reward and bond amount
    if staker_info.bond_amount.is_zero() {
        remove_staker_info(&mut deps.storage, &sender_addr_raw);
    } else {
        store_staker_info(&mut deps.storage, &sender_addr_raw, &staker_info)?;
    }

    // Store updated state
    store_state(&mut deps.storage, &state)?;
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.anchor_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: env.message.sender.clone(),
                amount,
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "withdraw"),
            log("owner", env.message.sender),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
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
    state.total_bond_amount = (state.total_bond_amount - amount)?;
    staker_info.bond_amount = (staker_info.bond_amount - amount)?;
    Ok(())
}

// compute distributed rewards and update global reward index
fn compute_reward(config: &Config, state: &mut State, block_height: u64) {
    if state.total_bond_amount.is_zero() {
        state.last_distributed = block_height;
        return;
    }

    let mut distributed_amount: Uint128 = Uint128::zero();
    for s in config.distribution_schedule.iter() {
        if s.0 > block_height || s.1 < state.last_distributed {
            continue;
        }

        // min(s.1, block_height) - max(s.0, last_distributed)
        let passed_blocks =
            std::cmp::min(s.1, block_height) - std::cmp::max(s.0, state.last_distributed);

        let num_blocks = s.1 - s.0;
        let distribution_amount_per_block: Decimal = Decimal::from_ratio(s.2, num_blocks);
        distributed_amount += distribution_amount_per_block * Uint128(passed_blocks as u128);
    }

    state.last_distributed = block_height;
    state.global_reward_index = state.global_reward_index
        + Decimal::from_ratio(distributed_amount, state.total_bond_amount);
}

// withdraw reward to pending reward
fn compute_staker_reward(state: &State, staker_info: &mut StakerInfo) -> StdResult<()> {
    let pending_reward = (staker_info.bond_amount * state.global_reward_index
        - staker_info.bond_amount * staker_info.reward_index)?;

    staker_info.reward_index = state.global_reward_index;
    staker_info.pending_reward += pending_reward;
    Ok(())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State { block_height } => to_binary(&query_state(deps, block_height)?),
        QueryMsg::StakerInfo {
            staker,
            block_height,
        } => to_binary(&query_staker_info(deps, staker, block_height)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        anchor_token: deps.api.human_address(&state.anchor_token)?,
        staking_token: deps.api.human_address(&state.staking_token)?,
        distribution_schedule: state.distribution_schedule,
    };

    Ok(resp)
}

pub fn query_state<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: Option<u64>,
) -> StdResult<StateResponse> {
    let mut state: State = read_state(&deps.storage)?;
    if let Some(block_height) = block_height {
        let config = read_config(&deps.storage)?;
        compute_reward(&config, &mut state, block_height);
    }

    Ok(StateResponse {
        last_distributed: state.last_distributed,
        total_bond_amount: state.total_bond_amount,
        global_reward_index: state.global_reward_index,
    })
}

pub fn query_staker_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    staker: HumanAddr,
    block_height: Option<u64>,
) -> StdResult<StakerInfoResponse> {
    let staker_raw = deps.api.canonical_address(&staker)?;

    let mut staker_info: StakerInfo = read_staker_info(&deps.storage, &staker_raw)?;
    if let Some(block_height) = block_height {
        let config = read_config(&deps.storage)?;
        let mut state = read_state(&deps.storage)?;

        compute_reward(&config, &mut state, block_height);
        compute_staker_reward(&state, &mut staker_info)?;
    }

    Ok(StakerInfoResponse {
        staker,
        reward_index: staker_info.reward_index,
        bond_amount: staker_info.bond_amount,
        pending_reward: staker_info.pending_reward,
    })
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
