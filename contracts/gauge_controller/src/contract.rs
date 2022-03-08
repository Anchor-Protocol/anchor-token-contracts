use crate::error::ContractError;
use crate::state::{
    config_read, config_store, gauge_addr_read, gauge_addr_store, gauge_count_read,
    gauge_count_store, gauge_info_read, gauge_info_store, gauge_weight_read, gauge_weight_store,
    Config, GaugeInfo, UserVote, Weight,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, CanonicalAddr, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
};

use anchor_token::gauge_controller::{
    AllGaugeAddrResponse, ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeCountResponse,
    GaugeWeightResponse, InstantiateMsg, QueryMsg, RelativeWeightResponse, TotalWeightResponse,
};

const WEEK: u64 = 7 * 24 * 60 * 60;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        owner: deps.api.addr_canonicalize(&msg.owner)?,
        anchor_token: deps.api.addr_canonicalize(&msg.anchor_token)?,
        anchor_voting_escorw: deps.api.addr_canonicalize(&msg.anchor_voting_escorw)?,
    };
    config_store(deps.storage).save(&config)?;
    gauge_count_store(deps.storage).save(&0)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddGauge { addr, weight } => add_gauge(deps, env, info, addr, weight),
        ExecuteMsg::ChangeGaugeWeight { addr, weight } => {
            change_gauge_weight(deps, env, info, addr, weight)
        }
        ExecuteMsg::VoteForGaugeWeight { addr, user_weight } => {
            vote_for_gauge_weight(deps, env, info, addr, user_weight)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GaugeCount {} => Ok(to_binary(&query_gauge_count(deps)?)?),
        QueryMsg::GaugeWeight { addr } => Ok(to_binary(&query_gauge_weight(deps, addr)?)?),
        QueryMsg::TotalWeight {} => Ok(to_binary(&query_total_weight(deps)?)?),
        QueryMsg::GaugeAddr { gauge_id } => Ok(to_binary(&query_gauge_addr(deps, gauge_id)?)?),
        QueryMsg::AllGaugeAddr {} => Ok(to_binary(&query_all_gauge_addr(deps)?)?),
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::RelativeWeight { addr, time } => {
            Ok(to_binary(&query_relative_weight(deps, addr, time)?)?)
        }
    }
}

fn _get_period(time: u64) -> u64 {
    (time / WEEK + WEEK) * WEEK
}

fn add_gauge(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    addr: String,
    weight: Uint128,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_canonicalize(&addr)?;
    if let Ok(_) = gauge_info_read(deps.storage).load(&addr) {
        return Err(ContractError::GaugeAlreadyExist {});
    }
    let mut gauge_count = gauge_count_read(deps.storage).load()?;
    gauge_addr_store(deps.storage).save(&gauge_count.to_string().as_bytes(), &addr)?;
    gauge_count += 1;
    gauge_count_store(deps.storage).save(&gauge_count)?;
    let period = _get_period(env.block.time.seconds());
    gauge_weight_store(deps.storage, &addr).save(
        &period.to_string().as_bytes(),
        &Weight {
            bias: weight,
            slope: Uint128::zero(),
            slope_change: Uint128::zero(),
        },
    )?;
    gauge_info_store(deps.storage).save(
        &addr,
        &GaugeInfo {
            last_vote_period: period,
        },
    )?;
    Ok(Response::default())
}

fn change_gauge_weight(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _addr: String,
    _weight: Uint128,
) -> Result<Response, ContractError> {
    Err(ContractError::NotImplement {})
}

fn vote_for_gauge_weight(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    addr: String,
    user_weight: Uint128,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_canonicalize(&addr)?;
    let period = _get_period(env.block.time.seconds());
    gauge_weight_store(deps.storage, &addr).save(
        &period.to_string().as_bytes(),
        &Weight {
            bias: user_weight,
            slope: Uint128::from(234_u64),
            slope_change: Uint128::from(345_u64),
        },
    )?;
    gauge_info_store(deps.storage).save(
        &addr,
        &GaugeInfo {
            last_vote_period: period,
        },
    )?;
    Ok(Response::default())
}

fn query_gauge_weight(deps: Deps, addr: String) -> Result<GaugeWeightResponse, ContractError> {
    let addr = deps.api.addr_canonicalize(&addr)?;
    let period = gauge_info_read(deps.storage).load(&addr)?.last_vote_period;
    Ok(GaugeWeightResponse {
        gauge_weight: gauge_weight_read(deps.storage, &addr)
            .load(&period.to_string().as_bytes())?
            .bias,
    })
}

fn query_total_weight(_deps: Deps) -> Result<TotalWeightResponse, ContractError> {
    Err(ContractError::NotImplement {})
}

fn query_relative_weight(
    _deps: Deps,
    _addr: String,
    _time: Uint128,
) -> Result<RelativeWeightResponse, ContractError> {
    Err(ContractError::NotImplement {})
}

fn query_gauge_count(deps: Deps) -> Result<GaugeCountResponse, ContractError> {
    Ok(GaugeCountResponse {
        gauge_count: gauge_count_read(deps.storage).load()?,
    })
}

fn query_gauge_addr(deps: Deps, gauge_id: u64) -> Result<GaugeAddrResponse, ContractError> {
    if gauge_id >= gauge_count_read(deps.storage).load()? {
        return Err(ContractError::GaugeNotFound {});
    }
    let gauge_addr = gauge_addr_read(deps.storage).load(&gauge_id.to_string().as_bytes())?;
    Ok(GaugeAddrResponse {
        gauge_addr: deps.api.addr_humanize(&gauge_addr)?.to_string(),
    })
}

fn query_all_gauge_addr(deps: Deps) -> Result<AllGaugeAddrResponse, ContractError> {
    let gauge_count = gauge_count_read(deps.storage).load()?;
    let mut all_gauge_addr = vec![];
    for i in 0..gauge_count {
        let gauge_addr = gauge_addr_read(deps.storage).load(&i.to_string().as_bytes())?;
        all_gauge_addr.push(deps.api.addr_humanize(&gauge_addr)?.to_string());
    }
    Ok(AllGaugeAddrResponse {
        all_gauge_addr: all_gauge_addr,
    })
}

fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = config_read(deps.storage).load()?;
    Ok(ConfigResponse {
        owner: deps.api.addr_humanize(&config.owner)?.to_string(),
        anchor_token: deps.api.addr_humanize(&config.anchor_token)?.to_string(),
        anchor_voting_escorw: deps
            .api
            .addr_humanize(&config.anchor_voting_escorw)?
            .to_string(),
    })
}
