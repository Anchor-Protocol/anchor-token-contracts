use crate::error::ContractError;
use crate::state::{
    GaugeWeight, UserSlopResponse, UserUnlockPeriodResponse, VotingEscrowContractQueryMsg, CONFIG,
    GAUGE_WEIGHT, SLOPE_CHANGES,
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

pub(crate) fn fetch_last_checkpoint(
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
    if let Ok(last_checkpoint) = fetch_last_checkpoint(storage, addr) {
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
