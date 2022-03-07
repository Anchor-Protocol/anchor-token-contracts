use crate::error::ContractError;
use crate::state::{
    config_read, config_store, gauge_addr_read, gauge_addr_store, gauge_info_read,
    gauge_info_store, gauge_weight_read, gauge_weight_store, n_gauges_read, n_gauges_store, Config,
    GaugeInfo, UserVote, Weight,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, CanonicalAddr, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
};

use anchor_token::gauge_controller::{
    ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeWeightResponse, InstantiateMsg,
    NGaugesResponse, QueryMsg, RelativeWeightResponse, TotalWeightResponse,
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
        voting_escrow_contract: msg.voting_escrow_contract,
    };
    config_store(deps.storage).save(&config)?;
    n_gauges_store(deps.storage).save(&0)?;
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
        QueryMsg::GetNGauges {} => Ok(to_binary(&get_n_gauges(deps)?)?),
        QueryMsg::GetGaugeWeight { addr } => Ok(to_binary(&get_gauge_weight(deps, addr)?)?),
        QueryMsg::GetTotalWeight {} => Ok(to_binary(&get_total_weight(deps)?)?),
        QueryMsg::GetGaugeAddr { gauge_id } => Ok(to_binary(&get_gauge_addr(deps, gauge_id)?)?),
        QueryMsg::GetConfig {} => Ok(to_binary(&get_config(deps)?)?),
        QueryMsg::GetRelativeWeight { addr, time } => {
            Ok(to_binary(&get_relative_weight(deps, addr, time)?)?)
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
    addr: CanonicalAddr,
    weight: Uint128,
) -> Result<Response, ContractError> {
    let mut n_gauges = n_gauges_read(deps.storage).load()?;
    gauge_addr_store(deps.storage).save(&n_gauges.to_string().as_bytes(), &addr)?;
    n_gauges += 1;
    n_gauges_store(deps.storage).save(&n_gauges)?;
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
    _addr: CanonicalAddr,
    _weight: Uint128,
) -> Result<Response, ContractError> {
    Err(ContractError::NotImplement {})
}

fn vote_for_gauge_weight(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    addr: CanonicalAddr,
    user_weight: Uint128,
) -> Result<Response, ContractError> {
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

fn get_gauge_weight(deps: Deps, addr: CanonicalAddr) -> Result<GaugeWeightResponse, ContractError> {
    let period = gauge_info_read(deps.storage).load(&addr)?.last_vote_period;
    Ok(GaugeWeightResponse {
        gauge_weight: gauge_weight_read(deps.storage, &addr)
            .load(&period.to_string().as_bytes())?
            .bias,
    })
}

fn get_total_weight(_deps: Deps) -> Result<TotalWeightResponse, ContractError> {
    Err(ContractError::NotImplement {})
}

fn get_relative_weight(
    _deps: Deps,
    _addr: CanonicalAddr,
    _time: Uint128,
) -> Result<RelativeWeightResponse, ContractError> {
    Err(ContractError::NotImplement {})
}

fn get_n_gauges(deps: Deps) -> Result<NGaugesResponse, ContractError> {
    Ok(NGaugesResponse {
        n_gauges: n_gauges_read(deps.storage).load()?,
    })
}

fn get_gauge_addr(deps: Deps, gauge_id: u64) -> Result<GaugeAddrResponse, ContractError> {
    if gauge_id >= n_gauges_read(deps.storage).load()? {
        return Err(ContractError::GaugeNotFound {});
    }
    Ok(GaugeAddrResponse {
        gauge_addr: gauge_addr_read(deps.storage).load(&gauge_id.to_string().as_bytes())?,
    })
}

fn get_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = config_read(deps.storage).load()?;
    Ok(ConfigResponse {
        voting_escrow_contract: config.voting_escrow_contract,
    })
}
