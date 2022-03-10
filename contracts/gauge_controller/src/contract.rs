use crate::error::ContractError;
use crate::state::{Config, GaugeWeight, CONFIG, GAUGE_ADDR, GAUGE_COUNT, GAUGE_WEIGHT};
use crate::utils::{
    calc_new_weight, check_if_exists, deserialize_pair, fetch_last_checkpoint, fetch_slope_changes,
    get_period, query_last_user_slope, query_user_unlock_period,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, Storage, Uint128,
};

use cw_storage_plus::U64Key;

use anchor_token::gauge_controller::{
    AllGaugeAddrResponse, ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeCountResponse,
    GaugeWeightResponse, InstantiateMsg, QueryMsg, RelativeWeightResponse, TotalWeightResponse,
};

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

fn checkpoint_gauge(
    storage: &mut dyn Storage,
    addr: &Addr,
    new_period: u64,
) -> Result<Response, ContractError> {
    let last_checkpoint = fetch_last_checkpoint(storage, &addr)?;

    if let Some(pair) = last_checkpoint {
        let (mut old_period, mut weight) = deserialize_pair::<GaugeWeight>(Ok(pair))?;

        // cannot happen
        if new_period < old_period {
            return Err(ContractError::TimestampError {});
        }

        // no need to do checkpoint
        if new_period == old_period {
            return Ok(Response::default());
        }

        let scheduled_slope_changes = fetch_slope_changes(storage, &addr, old_period, new_period)?;

        for (recalc_period, scheduled_change) in scheduled_slope_changes {
            assert!(recalc_period > old_period);

            let dt = recalc_period - old_period;

            weight = calc_new_weight(weight, dt, scheduled_change);

            GAUGE_WEIGHT.save(storage, (addr.clone(), U64Key::new(recalc_period)), &weight)?;

            old_period = recalc_period;
        }

        let dt = new_period - old_period;

        if dt > 0 {
            GAUGE_WEIGHT.save(
                storage,
                (addr.clone(), U64Key::new(new_period)),
                &calc_new_weight(weight, dt, Decimal::zero()),
            )?;
        }
    } else {
        return Err(ContractError::GaugeNotFound {});
    }
    Ok(Response::default())
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
            slope: Decimal::zero(),
        },
    )?;

    let slope = query_last_user_slope(deps.as_ref(), sender.clone())?;
    assert_eq!(Decimal::from_ratio(2_u64, 3_u64), slope);

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
    let period = get_period(env.block.time.seconds());

    checkpoint_gauge(deps.storage, &addr, period)?;

    let last_checkpoint = fetch_last_checkpoint(deps.storage, &addr)?;

    if let Some(pair) = last_checkpoint {
        let (last_period, last_weight) = deserialize_pair::<GaugeWeight>(Ok(pair))?;

        if last_period != period {
            return Err(ContractError::TimestampError {});
        }

        GAUGE_WEIGHT.save(
            deps.storage,
            (addr.clone(), U64Key::new(period)),
            &GaugeWeight {
                bias: weight,
                slope: last_weight.slope,
            },
        )?;
    } else {
        return Err(ContractError::GaugeNotFound {});
    }
    Ok(Response::default())
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
            slope: Decimal::zero(),
        },
    )?;

    Ok(Response::default())
}

fn query_gauge_weight(deps: Deps, addr: String) -> Result<GaugeWeightResponse, ContractError> {
    let addr = deps.api.addr_validate(&addr)?;
    let last_checkpoint = fetch_last_checkpoint(deps.storage, &addr)?;

    if let Some(pair) = last_checkpoint {
        let (_, last_weight) = deserialize_pair::<GaugeWeight>(Ok(pair))?;
        return Ok(GaugeWeightResponse {
            gauge_weight: last_weight.bias,
        });
    } else {
        return Err(ContractError::GaugeNotFound {});
    }
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
