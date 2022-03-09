use crate::error::ContractError;
use crate::state::{
    Config, GaugeInfo, GaugeWeight, UserSlopResponse, UserUnlockPeriodResponse,
    VotingEscrowContractQueryMsg, CONFIG, GAUGE_ADDR, GAUGE_COUNT, GAUGE_INFO, GAUGE_WEIGHT,
    USER_VOTES,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response, Uint128,
    WasmQuery,
};

use cw_storage_plus::U64Key;

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
    CONFIG.save(
        deps.storage,
        &Config {
            owner: deps.api.addr_validate(&msg.owner)?,
            anchor_token: deps.api.addr_validate(&msg.anchor_token)?,
            anchor_voting_escorw: deps.api.addr_validate(&msg.anchor_voting_escorw)?,
        },
    )?;
    GAUGE_COUNT.save(deps.storage, &0)?;
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
        QueryMsg::GaugeRelativeWeight { addr, time } => {
            Ok(to_binary(&query_relative_weight(deps, addr, time)?)?)
        }
    }
}

fn get_period(seconds: u64) -> u64 {
    (seconds / WEEK + WEEK) * WEEK
}

fn query_last_user_slope(deps: Deps, user: Addr) -> Result<Uint128, ContractError> {
    let anchor_voting_escorw = CONFIG.load(deps.storage)?.anchor_voting_escorw;
    Ok(deps
        .querier
        .query::<UserSlopResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: anchor_voting_escorw.to_string(),
            msg: to_binary(&VotingEscrowContractQueryMsg::LastUserSlope {
                user: user.to_string(),
            })?,
        }))?
        .slope)
}

fn query_user_unlock_period(deps: Deps, user: Addr) -> Result<u64, ContractError> {
    let anchor_voting_escorw = CONFIG.load(deps.storage)?.anchor_voting_escorw;
    Ok(deps
        .querier
        .query::<UserUnlockPeriodResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: anchor_voting_escorw.to_string(),
            msg: to_binary(&VotingEscrowContractQueryMsg::UserUnlockPeriod {
                user: user.to_string(),
            })?,
        }))?
        .unlock_period)
}

fn check_if_exists(deps: Deps, gauge_addr: &Addr) -> bool {
    match GAUGE_INFO.load(deps.storage, gauge_addr.clone()) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn add_gauge(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    addr: String,
    weight: Uint128,
) -> Result<Response, ContractError> {
    let sender = info.sender;

    if CONFIG.load(deps.storage)?.owner != sender {
        return Err(ContractError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&addr)?;

    if check_if_exists(deps.as_ref(), &addr) {
        return Err(ContractError::GaugeAlreadyExist {});
    }

    let gauge_count = GAUGE_COUNT.load(deps.storage)?;

    GAUGE_ADDR.save(deps.storage, U64Key::new(gauge_count), &addr)?;

    GAUGE_COUNT.save(deps.storage, &(gauge_count + 1))?;

    let period = get_period(env.block.time.seconds());

    GAUGE_WEIGHT.save(
        deps.storage,
        (addr.clone(), U64Key::new(period)),
        &GaugeWeight {
            bias: weight,
            slope: Uint128::zero(),
            slope_change: Uint128::zero(),
        },
    )?;

    GAUGE_INFO.save(
        deps.storage,
        addr.clone(),
        &GaugeInfo {
            last_vote_period: period,
        },
    )?;

    let slope = query_last_user_slope(deps.as_ref(), sender.clone())?;
    assert_eq!(Uint128::from(233_u64), slope);

    let unlock_period = query_user_unlock_period(deps.as_ref(), sender.clone())?;
    assert_eq!(666, unlock_period);

    Ok(Response::default())
}

fn change_gauge_weight(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    addr: String,
    weight: Uint128,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_validate(info.sender.as_str())?;

    if CONFIG.load(deps.storage)?.owner != sender {
        return Err(ContractError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&addr)?;

    if let Ok(old_info) = GAUGE_INFO.load(deps.storage, addr.clone()) {
        let old_period = old_info.last_vote_period;
        let old_weight =
            GAUGE_WEIGHT.load(deps.storage, (addr.clone(), U64Key::new(old_period)))?;
        let new_period = get_period(env.block.time.seconds());

        GAUGE_WEIGHT.save(
            deps.storage,
            (addr.clone(), U64Key::new(new_period)),
            &GaugeWeight {
                bias: weight,
                slope: old_weight.slope,
                slope_change: old_weight.slope_change,
            },
        )?;

        GAUGE_INFO.save(
            deps.storage,
            addr.clone(),
            &GaugeInfo {
                last_vote_period: new_period,
            },
        )?;

        return Ok(Response::default());
    }

    Err(ContractError::GaugeNotFound {})
}

fn vote_for_gauge_weight(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    addr: String,
    user_weight: Uint128,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(&addr)?;
    let period = get_period(env.block.time.seconds());

    GAUGE_WEIGHT.save(
        deps.storage,
        (addr.clone(), U64Key::new(period)),
        &GaugeWeight {
            bias: user_weight,
            slope: Uint128::from(234_u64),
            slope_change: Uint128::from(345_u64),
        },
    )?;

    GAUGE_INFO.save(
        deps.storage,
        addr.clone(),
        &GaugeInfo {
            last_vote_period: period,
        },
    )?;

    Ok(Response::default())
}

fn query_gauge_weight(deps: Deps, addr: String) -> Result<GaugeWeightResponse, ContractError> {
    let addr = deps.api.addr_validate(&addr)?;
    let period = GAUGE_INFO
        .load(deps.storage, addr.clone())?
        .last_vote_period;

    Ok(GaugeWeightResponse {
        gauge_weight: GAUGE_WEIGHT
            .load(deps.storage, (addr.clone(), U64Key::new(period)))?
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
        gauge_count: GAUGE_COUNT.load(deps.storage)?,
    })
}

fn query_gauge_addr(deps: Deps, gauge_id: u64) -> Result<GaugeAddrResponse, ContractError> {
    if gauge_id >= GAUGE_COUNT.load(deps.storage)? {
        return Err(ContractError::GaugeNotFound {});
    }

    let gauge_addr = GAUGE_ADDR.load(deps.storage, U64Key::new(gauge_id))?;

    Ok(GaugeAddrResponse {
        gauge_addr: gauge_addr.to_string(),
    })
}

fn query_all_gauge_addr(deps: Deps) -> Result<AllGaugeAddrResponse, ContractError> {
    let gauge_count = GAUGE_COUNT.load(deps.storage)?;
    let mut all_gauge_addr = vec![];

    for i in 0..gauge_count {
        let gauge_addr = GAUGE_ADDR.load(deps.storage, U64Key::new(i))?;
        all_gauge_addr.push(gauge_addr.to_string());
    }

    Ok(AllGaugeAddrResponse {
        all_gauge_addr: all_gauge_addr,
    })
}

fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        anchor_token: config.anchor_token.to_string(),
        anchor_voting_escorw: config.anchor_voting_escorw.to_string(),
    })
}
