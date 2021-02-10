use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, MigrateResponse, MigrateResult, Querier,
    StdError, StdResult, Storage, Uint128, WasmMsg,
};

use anchor_token::staking::{
    ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, MigrateMsg, PoolInfoResponse, QueryMsg,
    RewardInfoResponse,
};

use crate::state::{
    read_config, read_pool_info, read_reward_info, remove_reward_info, store_config,
    store_pool_info, store_reward_info, Config, PoolInfo, RewardInfo,
};

use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&msg.owner)?,
            anchor_token: deps.api.canonical_address(&msg.anchor_token)?,
            staking_token: deps.api.canonical_address(&msg.staking_token)?,
        },
    )?;

    store_pool_info(
        &mut deps.storage,
        &PoolInfo {
            pending_reward: Uint128::zero(),
            total_bond_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
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
        HandleMsg::UpdateConfig { owner } => update_config(deps, env, owner),
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
            Cw20HookMsg::DepositReward {} => {
                // only reward token contract can execute this message
                if config.anchor_token != deps.api.canonical_address(&env.message.sender)? {
                    return Err(StdError::unauthorized());
                }

                deposit_reward(deps, env, cw20_msg.amount)
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
) -> StdResult<HandleResponse> {
    let mut config: Config = read_config(&deps.storage)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

pub fn bond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    sender_addr: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let sender_addr_raw: CanonicalAddr = deps.api.canonical_address(&sender_addr)?;
    let mut pool_info: PoolInfo = read_pool_info(&deps.storage)?;
    let mut reward_info: RewardInfo = read_reward_info(&deps.storage, &sender_addr_raw)?;

    // Withdraw reward to pending reward; before changing share
    before_share_change(&pool_info, &mut reward_info)?;

    // Increase bond_amount
    increase_bond_amount(&mut pool_info, &mut reward_info, amount);

    store_reward_info(&mut deps.storage, &sender_addr_raw, &reward_info)?;
    store_pool_info(&mut deps.storage, &pool_info)?;

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

    let mut pool_info: PoolInfo = read_pool_info(&deps.storage)?;
    let mut reward_info: RewardInfo = read_reward_info(&deps.storage, &sender_addr_raw)?;

    if reward_info.bond_amount < amount {
        return Err(StdError::generic_err("Cannot unbond more than bond amount"));
    }

    // Distribute reward to pending reward; before changing share
    before_share_change(&pool_info, &mut reward_info)?;

    // Decrease bond_amount
    decrease_bond_amount(&mut pool_info, &mut reward_info, amount)?;

    // Update rewards info
    if reward_info.pending_reward.is_zero() && reward_info.bond_amount.is_zero() {
        remove_reward_info(&mut deps.storage, &sender_addr_raw);
    } else {
        store_reward_info(&mut deps.storage, &sender_addr_raw, &reward_info)?;
    }

    // Update pool info
    store_pool_info(&mut deps.storage, &pool_info)?;

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

// deposit reward must be from reward token contract
pub fn deposit_reward<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    amount: Uint128,
) -> HandleResult {
    let mut pool_info: PoolInfo = read_pool_info(&deps.storage)?;
    if pool_info.total_bond_amount.is_zero() {
        pool_info.pending_reward += amount;
    } else {
        let reward_per_bond = Decimal::from_ratio(
            amount + pool_info.pending_reward,
            pool_info.total_bond_amount,
        );
        pool_info.reward_index = pool_info.reward_index + reward_per_bond;
        pool_info.pending_reward = Uint128::zero();
    }

    store_pool_info(&mut deps.storage, &pool_info)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "deposit_reward"),
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
    let mut reward_info = read_reward_info(&deps.storage, &sender_addr_raw)?;

    // single reward withdraw
    let pool_info: PoolInfo = read_pool_info(&deps.storage)?;

    // Withdraw reward to pending reward
    before_share_change(&pool_info, &mut reward_info)?;

    let amount = reward_info.pending_reward;
    reward_info.pending_reward = Uint128::zero();

    // Update rewards info
    if reward_info.bond_amount.is_zero() {
        remove_reward_info(&mut deps.storage, &sender_addr_raw);
    } else {
        store_reward_info(&mut deps.storage, &sender_addr_raw, &reward_info)?;
    }

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

fn increase_bond_amount(pool_info: &mut PoolInfo, reward_info: &mut RewardInfo, amount: Uint128) {
    pool_info.total_bond_amount += amount;
    reward_info.bond_amount += amount;
}

fn decrease_bond_amount(
    pool_info: &mut PoolInfo,
    reward_info: &mut RewardInfo,
    amount: Uint128,
) -> StdResult<()> {
    pool_info.total_bond_amount = (pool_info.total_bond_amount - amount)?;
    reward_info.bond_amount = (reward_info.bond_amount - amount)?;
    Ok(())
}

// withdraw reward to pending reward
fn before_share_change(pool_info: &PoolInfo, reward_info: &mut RewardInfo) -> StdResult<()> {
    let pending_reward = (reward_info.bond_amount * pool_info.reward_index
        - reward_info.bond_amount * reward_info.index)?;

    reward_info.index = pool_info.reward_index;
    reward_info.pending_reward += pending_reward;
    Ok(())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PoolInfo {} => to_binary(&query_pool_info(deps)?),
        QueryMsg::RewardInfo { staker } => to_binary(&query_reward_info(deps, staker)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        anchor_token: deps.api.human_address(&state.anchor_token)?,
        staking_token: deps.api.human_address(&state.staking_token)?,
    };

    Ok(resp)
}

pub fn query_pool_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<PoolInfoResponse> {
    let pool_info: PoolInfo = read_pool_info(&deps.storage)?;
    Ok(PoolInfoResponse {
        total_bond_amount: pool_info.total_bond_amount,
        reward_index: pool_info.reward_index,
        pending_reward: pool_info.pending_reward,
    })
}

pub fn query_reward_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    staker: HumanAddr,
) -> StdResult<RewardInfoResponse> {
    let staker_raw = deps.api.canonical_address(&staker)?;

    let reward_info: RewardInfo = read_reward_info(&deps.storage, &staker_raw)?;

    Ok(RewardInfoResponse {
        staker,
        index: reward_info.index,
        bond_amount: reward_info.bond_amount,
        pending_reward: reward_info.pending_reward,
    })
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
