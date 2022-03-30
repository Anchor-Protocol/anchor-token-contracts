use crate::error::ContractError;
use cosmwasm_std::{
    Addr, Api, Decimal, Deps, DepsMut, Fraction, Order, OverflowError, Pair, StdError, StdResult,
    Uint128, Uint256,
};
use cw_storage_plus::{Bound, U64Key};
use std::convert::TryInto;

use crate::state::{Point, HISTORY, LAST_SLOPE_CHANGE, SLOPE_CHANGES};

/// Seconds in one week. Constant is intended for period number calculation.
pub const WEEK: u64 = 7 * 86400; // lock period is rounded down by week

/// Seconds in 1 year which is minimum lock period.
pub const MIN_LOCK_TIME: u64 = 365 * 86400; // 1 year

/// Seconds in 2 years which is maximum lock period.
pub const MAX_LOCK_TIME: u64 = 4 * 365 * 86400; // 4 years

/// # Description
/// Checks the time is within limits
pub(crate) fn time_limits_check(time: u64) -> Result<(), ContractError> {
    if !(MIN_LOCK_TIME..=MAX_LOCK_TIME).contains(&time) {
        Err(ContractError::LockTimeLimitsError {})
    } else {
        Ok(())
    }
}

/// # Description
/// Trait is intended for Decimal rounding problem elimination
trait DecimalRoundedCheckedMul {
    fn checked_mul(self, other: Uint128) -> Result<Uint128, OverflowError>;
}

impl DecimalRoundedCheckedMul for Decimal {
    fn checked_mul(self, other: Uint128) -> Result<Uint128, OverflowError> {
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

/// # Description
/// Main calculation function by formula: previous_power - slope*(x - previous_x)
pub(crate) fn calc_voting_power(point: &Point, period: u64) -> Uint128 {
    let shift = point
        .slope
        .checked_mul(Uint128::from(period - point.start))
        .unwrap_or_else(|_| Uint128::zero());
    point
        .power
        .checked_sub(shift)
        .unwrap_or_else(|_| Uint128::zero())
}

/// # Description
/// Coefficient calculation where 0 [`WEEK`] equals to 1 and [`MAX_LOCK_TIME`] equals to 2.5.
pub(crate) fn calc_coefficient(interval: u64) -> Decimal {
    // coefficient = 2.5 * (end - start) / MAX_LOCK_TIME
    Decimal::from_ratio(25_u64 * interval, get_period(MAX_LOCK_TIME) * 10)
}

/// # Description
/// Fetches last checkpoint in [`HISTORY`] for given address.
pub(crate) fn fetch_last_checkpoint(
    deps: Deps,
    addr: &Addr,
    period_key: &U64Key,
) -> StdResult<Option<Pair<Point>>> {
    HISTORY
        .prefix(addr.clone())
        .range(
            deps.storage,
            None,
            Some(Bound::Inclusive(period_key.wrapped.clone())),
            Order::Descending,
        )
        .next()
        .transpose()
}

pub(crate) fn cancel_scheduled_slope(deps: DepsMut, slope: Decimal, period: u64) -> StdResult<()> {
    let end_period_key = U64Key::new(period);
    let last_slope_change = LAST_SLOPE_CHANGE
        .may_load(deps.as_ref().storage)?
        .unwrap_or(0);
    match SLOPE_CHANGES.may_load(deps.as_ref().storage, end_period_key.clone())? {
        // we do not need to schedule slope change in the past
        Some(old_scheduled_change) if period > last_slope_change => {
            let new_slope = old_scheduled_change - slope;
            if !new_slope.is_zero() {
                SLOPE_CHANGES.save(
                    deps.storage,
                    end_period_key,
                    &(old_scheduled_change - slope),
                )
            } else {
                SLOPE_CHANGES.remove(deps.storage, end_period_key);
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

pub(crate) fn schedule_slope_change(deps: DepsMut, slope: Decimal, period: u64) -> StdResult<()> {
    if !slope.is_zero() {
        SLOPE_CHANGES
            .update(
                deps.storage,
                U64Key::new(period),
                |slope_opt| -> StdResult<Decimal> {
                    if let Some(pslope) = slope_opt {
                        Ok(pslope + slope)
                    } else {
                        Ok(slope)
                    }
                },
            )
            .map(|_| ())
    } else {
        Ok(())
    }
}

/// # Description
/// Helper function for deserialization
pub(crate) fn deserialize_pair(pair: StdResult<Pair<Decimal>>) -> StdResult<(u64, Decimal)> {
    let (period_serialized, change) = pair?;
    let period_bytes: [u8; 8] = period_serialized
        .try_into()
        .map_err(|_| StdError::generic_err("Deserialization error"))?;
    Ok((u64::from_be_bytes(period_bytes), change))
}

/// # Description
/// Fetches all slope changes between last_slope_change and period.
pub(crate) fn fetch_slope_changes(
    deps: Deps,
    last_slope_change: u64,
    period: u64,
) -> StdResult<Vec<(u64, Decimal)>> {
    SLOPE_CHANGES
        .range(
            deps.storage,
            Some(Bound::Exclusive(U64Key::new(last_slope_change).wrapped)),
            Some(Bound::Inclusive(U64Key::new(period).wrapped)),
            Order::Ascending,
        )
        .map(deserialize_pair)
        .collect()
}

/// # Description
/// Calculates how many periods are within specified time. Time should be in seconds.
pub fn get_period(time: u64) -> u64 {
    time / WEEK
}

/// ## Description
/// Returns a lowercased, validated address upon success. Otherwise returns [`Err`]
/// ## Params
/// * **api** is an object of type [`Api`]
///
/// * **addr** is an object of type [`Addr`]
pub fn addr_validate_to_lower(api: &dyn Api, addr: &str) -> StdResult<Addr> {
    if addr.to_lowercase() != addr {
        return Err(StdError::generic_err(format!(
            "Address {} should be lowercase",
            addr
        )));
    }
    api.addr_validate(addr)
}
