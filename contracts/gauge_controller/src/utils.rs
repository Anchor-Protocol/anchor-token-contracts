use crate::error::ContractError;
use crate::state::{
    GaugeWeight, UserSlopResponse, UserUnlockPeriodResponse, VotingEscrowContractQueryMsg, CONFIG,
    GAUGE_ADDR, GAUGE_COUNT, GAUGE_WEIGHT, SLOPE_CHANGES,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    to_binary, Addr, Decimal, Deps, Fraction, Order, OverflowError, Pair, QueryRequest, Response,
    StdResult, Storage, Uint128, Uint256, WasmQuery,
};

use cw_storage_plus::{Bound, U64Key};

use std::cmp::max;
use std::convert::TryInto;

pub(crate) const DAY: u64 = 24 * 60 * 60;
pub(crate) const WEEK: u64 = 7 * DAY;
pub(crate) const VOTE_DELAY: u64 = 2;
const MAX_PERIOD: u64 = u64::MAX;

pub(crate) fn get_period(seconds: u64) -> u64 {
    seconds / WEEK
}

pub(crate) fn query_last_user_slope(deps: Deps, user: Addr) -> Result<Decimal, ContractError> {
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

pub(crate) fn query_user_unlock_period(deps: Deps, user: Addr) -> Result<u64, ContractError> {
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

pub(crate) fn fetch_lastest_checkpoint(
    storage: &dyn Storage,
    addr: &Addr,
) -> Result<Option<Pair<GaugeWeight>>, ContractError> {
    GAUGE_WEIGHT
        .prefix(addr.clone())
        .range(
            storage,
            None,
            Some(Bound::Inclusive(U64Key::new(MAX_PERIOD).wrapped.clone())),
            Order::Descending,
        )
        .next()
        .transpose()
        .map_err(|_| ContractError::DeserializationError {})
}

pub(crate) fn fetch_slope_changes(
    storage: &dyn Storage,
    addr: &Addr,
    from_period: u64,
    to_period: u64,
) -> Result<Vec<(u64, Decimal)>, ContractError> {
    SLOPE_CHANGES
        .prefix(addr.clone())
        .range(
            storage,
            Some(Bound::Exclusive(U64Key::new(from_period).wrapped)),
            Some(Bound::Inclusive(U64Key::new(to_period).wrapped)),
            Order::Ascending,
        )
        .map(deserialize_pair::<Decimal>)
        .collect()
}

pub(crate) fn cancel_scheduled_slope_change(
    storage: &mut dyn Storage,
    addr: &Addr,
    slope: Decimal,
    period: u64,
) -> Result<Response, ContractError> {
    if slope.is_zero() {
        return Ok(Response::default());
    }

    let key = (addr.clone(), U64Key::new(period));

    if let Some(old_scheduled_slope_change) = SLOPE_CHANGES.may_load(storage, key.clone())? {
        let new_slope = max(old_scheduled_slope_change - slope, Decimal::zero());
        if new_slope.is_zero() {
            SLOPE_CHANGES.remove(storage, key.clone());
        } else {
            SLOPE_CHANGES.save(storage, key.clone(), &new_slope)?;
        }
    }

    Ok(Response::default())
}

pub(crate) fn schedule_slope_change(
    storage: &mut dyn Storage,
    addr: &Addr,
    slope: Decimal,
    period: u64,
) -> Result<Response, ContractError> {
    if slope.is_zero() {
        return Ok(Response::default());
    }

    SLOPE_CHANGES.update(
        storage,
        (addr.clone(), U64Key::new(period)),
        |slope_opt| -> Result<Decimal, ContractError> {
            if let Some(pslope) = slope_opt {
                Ok(pslope + slope)
            } else {
                Ok(slope)
            }
        },
    )?;

    Ok(Response::default())
}

pub(crate) fn deserialize_pair<T>(pair: StdResult<Pair<T>>) -> Result<(u64, T), ContractError> {
    let (period_serialized, change) = pair?;
    let period_bytes: [u8; 8] = period_serialized
        .try_into()
        .map_err(|_| ContractError::DeserializationError {})?;
    Ok((u64::from_be_bytes(period_bytes), change))
}

pub(crate) fn check_if_exists(storage: &dyn Storage, addr: &Addr) -> bool {
    if let Ok(last_checkpoint) = fetch_lastest_checkpoint(storage, addr) {
        if let Some(_) = last_checkpoint {
            return true;
        }
    }
    return false;
}

/// # Description
/// Trait is intended for Decimal rounding problem elimination
pub(crate) trait DecimalRoundedCheckedMul {
    fn checked_mul(self, other: u64) -> Result<Uint128, OverflowError>;
}

impl DecimalRoundedCheckedMul for Decimal {
    fn checked_mul(self, other: u64) -> Result<Uint128, OverflowError> {
        let other = Uint128::from(other);
        if self.is_zero() || other.is_zero() {
            return Ok(Uint128::zero());
        }
        let numerator = other.full_mul(self.numerator());
        let multiply_ratio = numerator / Uint256::from(self.denominator());
        if multiply_ratio > Uint256::from(Uint128::MAX) {
            Err(OverflowError::new(
                cosmwasm_std::OverflowOperation::Mul,
                self,
                other,
            ))
        } else {
            let mut result: Uint128 = multiply_ratio.try_into().unwrap();
            let rem: Uint128 = numerator
                .checked_rem(Uint256::from(self.denominator()))
                .unwrap()
                .try_into()
                .unwrap();
            // 0.5 in Decimal
            if rem.u128() >= 500000000000000000_u128 {
                result += Uint128::from(1_u128);
            }
            Ok(result)
        }
    }
}

pub(crate) fn calc_new_weight(weight: GaugeWeight, dt: u64, slope_change: Decimal) -> GaugeWeight {
    let slope = weight.slope;

    GaugeWeight {
        bias: weight.bias.saturating_sub(slope.checked_mul(dt).unwrap()),
        slope: max(slope - slope_change, Decimal::zero()),
    }
}

fn fetch_lastest_checkpoint_before(
    storage: &dyn Storage,
    addr: &Addr,
    period: u64,
) -> Result<Option<Pair<GaugeWeight>>, ContractError> {
    GAUGE_WEIGHT
        .prefix(addr.clone())
        .range(
            storage,
            None,
            Some(Bound::Inclusive(U64Key::new(period).wrapped.clone())),
            Order::Descending,
        )
        .next()
        .transpose()
        .map_err(|_| ContractError::DeserializationError {})
}

pub(crate) fn get_gauge_weight_at(
    storage: &dyn Storage,
    addr: &Addr,
    time: u64,
) -> Result<Uint128, ContractError> {
    let period = get_period(time);

    let lastest_checkpoint_before_period = fetch_lastest_checkpoint_before(storage, &addr, period)?;

    if let Some(pair) = lastest_checkpoint_before_period {
        let (mut old_period, mut weight) = deserialize_pair::<GaugeWeight>(Ok(pair))?;

        if old_period == period {
            return Ok(weight.bias);
        }

        let scheduled_slope_changes = fetch_slope_changes(storage, &addr, old_period, period)?;

        for (recalc_period, scheduled_change) in scheduled_slope_changes {
            assert!(recalc_period > old_period);
            let dt = recalc_period - old_period;
            weight = calc_new_weight(weight, dt, scheduled_change);
            old_period = recalc_period;
        }

        let dt = period - old_period;

        if dt > 0 {
            weight = calc_new_weight(weight, dt, Decimal::zero());
        }

        return Ok(weight.bias);
    }

    Err(ContractError::GaugeNotFound {})
}

pub(crate) fn get_total_weight_at(
    storage: &dyn Storage,
    time: u64,
) -> Result<Uint128, ContractError> {
    let gauge_count = GAUGE_COUNT.load(storage)?;
    let mut total_weight = Uint128::zero();

    for i in 0..gauge_count {
        let addr = GAUGE_ADDR.load(storage, U64Key::new(i))?;
        total_weight += get_gauge_weight_at(storage, &addr, time)?;
    }

    Ok(total_weight)
}

// Fill historic gauge weights week-over-week for missed checkins.
pub(crate) fn checkpoint_gauge(
    storage: &mut dyn Storage,
    addr: &Addr,
    new_period: u64,
) -> Result<Response, ContractError> {
    let lastest_checkpoint = fetch_lastest_checkpoint(storage, &addr)?;

    if let Some(pair) = lastest_checkpoint {
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
            old_period = recalc_period;

            GAUGE_WEIGHT.save(storage, (addr.clone(), U64Key::new(recalc_period)), &weight)?;
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
