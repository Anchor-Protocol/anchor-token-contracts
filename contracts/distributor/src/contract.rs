#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use crate::state::{read_config, read_state, store_config, store_state, Config, State};

use cosmwasm_std::{
    to_binary, Binary, CanonicalAddr, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128, WasmMsg,
};

use anchor_token::distributor::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StateResponse,
    TotalRewardsResponse,
};

use anchor_token::querier::query_token_balance;
use cosmwasm_bignumber::Uint256;
use cw20::Cw20ExecuteMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let whitelist = msg
        .whitelist
        .into_iter()
        .map(|w| deps.api.addr_canonicalize(&w))
        .collect::<StdResult<Vec<CanonicalAddr>>>()?;

    store_config(
        deps.storage,
        &Config {
            gov_contract: deps.api.addr_canonicalize(&msg.gov_contract)?,
            anchor_token: deps.api.addr_canonicalize(&msg.anchor_token)?,
            whitelist,
            spend_limit: msg.spend_limit,
        },
    )?;

    store_state(
        deps.storage,
        &State {
            paid_rewards: Uint256::zero(),
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { spend_limit } => update_config(deps, info, spend_limit),
        ExecuteMsg::Spend { recipient, amount } => spend(deps, info, recipient, amount),
        ExecuteMsg::AddDistributor { distributor } => add_distributor(deps, info, distributor),
        ExecuteMsg::RemoveDistributor { distributor } => {
            remove_distributor(deps, info, distributor)
        }
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    spend_limit: Option<Uint128>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;
    if config.gov_contract != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(spend_limit) = spend_limit {
        config.spend_limit = spend_limit;
    }

    store_config(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![("action", "update_config")]))
}

pub fn add_distributor(
    deps: DepsMut,
    info: MessageInfo,
    distributor: String,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;
    if config.gov_contract != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    let distributor_raw = deps.api.addr_canonicalize(&distributor)?;
    if config
        .whitelist
        .clone()
        .into_iter()
        .any(|w| w == distributor_raw)
    {
        return Err(StdError::generic_err("Distributor already registered"));
    }

    config.whitelist.push(distributor_raw);
    store_config(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "add_distributor"),
        ("distributor", distributor.as_str()),
    ]))
}

pub fn remove_distributor(
    deps: DepsMut,
    info: MessageInfo,
    distributor: String,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;
    if config.gov_contract != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    let distributor_raw = deps.api.addr_canonicalize(&distributor)?;
    let whitelist_len = config.whitelist.len();
    let whitelist: Vec<CanonicalAddr> = config
        .whitelist
        .into_iter()
        .filter(|w| *w != distributor_raw)
        .collect();

    if whitelist_len == whitelist.len() {
        return Err(StdError::generic_err("Distributor not found"));
    }

    config.whitelist = whitelist;
    store_config(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "remove_distributor"),
        ("distributor", distributor.as_str()),
    ]))
}

/// Spend
/// Owner can execute spend operation to send
/// `amount` of MIR token to `recipient` for community purpose
pub fn spend(
    deps: DepsMut,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let mut state: State = read_state(deps.storage)?;
    let sender_raw = deps.api.addr_canonicalize(info.sender.as_str())?;

    if !config.whitelist.into_iter().any(|w| w == sender_raw) {
        return Err(StdError::generic_err("unauthorized"));
    }

    if config.spend_limit < amount {
        return Err(StdError::generic_err("Cannot spend more than spend_limit"));
    }

    state.paid_rewards += amount.into();
    store_state(deps.storage, &state)?;

    let anchor_token = deps.api.addr_humanize(&config.anchor_token)?.to_string();
    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: anchor_token,
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient.clone(),
                amount,
            })?,
        })])
        .add_attributes(vec![
            ("action", "spend"),
            ("recipient", recipient.as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::TotalRewards {} => to_binary(&query_initial_balance(deps, env)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        gov_contract: deps.api.addr_humanize(&state.gov_contract)?.to_string(),
        anchor_token: deps.api.addr_humanize(&state.anchor_token)?.to_string(),
        whitelist: state
            .whitelist
            .into_iter()
            .map(|w| match deps.api.addr_humanize(&w) {
                Ok(addr) => Ok(addr.to_string()),
                Err(e) => Err(e),
            })
            .collect::<StdResult<Vec<String>>>()?,
        spend_limit: state.spend_limit,
    };

    Ok(resp)
}

pub fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = read_state(deps.storage)?;
    let res = StateResponse {
        paid_rewards: state.paid_rewards,
    };
    Ok(res)
}

pub fn query_initial_balance(deps: Deps, env: Env) -> StdResult<TotalRewardsResponse> {
    let state = read_state(deps.storage)?;
    let config = read_config(deps.storage)?;
    let balance = query_token_balance(
        deps,
        deps.api.addr_humanize(&config.anchor_token)?,
        env.contract.address,
    )?;

    let res = TotalRewardsResponse {
        total_rewards: state.paid_rewards + balance,
    };
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> StdResult<Response> {
    store_state(
        deps.storage,
        &State {
            paid_rewards: msg.paid_rewards,
        },
    )?;
    Ok(Response::default())
}
