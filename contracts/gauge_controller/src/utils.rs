use crate::error::ContractError;
use crate::state::{
    GaugeWeight, UserSlopResponse, UserUnlockPeriodResponse, VotingEscrowContractQueryMsg, CONFIG,
    GAUGE_WEIGHT, SLOPE_CHANGES, USER_VOTES,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    to_binary, Addr, Decimal, Deps, Fraction, Order, OverflowError, Pair, QueryRequest, StdResult,
    Storage, Uint128, Uint256, WasmQuery,
};

use cw_storage_plus::{Bound, U64Key};

use std::cmp::max;
use std::convert::TryInto;

const WEEK: u64 = 7 * 24 * 60 * 60;
const MAX_PERIOD: u64 = u64::MAX;

pub(crate) fn get_period(seconds: u64) -> u64 {
    (seconds / WEEK + WEEK) * WEEK
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

pub(crate) fn deserialize_pair<T>(pair: StdResult<Pair<T>>) -> Result<(u64, T), ContractError> {
    let (period_serialized, change) = pair?;
    let period_bytes: [u8; 8] = period_serialized
        .try_into()
        .map_err(|_| ContractError::DeserializationError {})?;
    Ok((u64::from_be_bytes(period_bytes), change))
}

pub(crate) fn check_if_exists(deps: Deps, addr: &Addr) -> bool {
    if let Ok(last_checkpoint) = fetch_last_checkpoint(deps.storage, addr) {
        if let Some(_) = last_checkpoint {
            return true;
        }
    }
    return false;
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

pub(crate) fn calc_new_weight(weight: GaugeWeight, dt: u64, slope_change: Decimal) -> GaugeWeight {
    GaugeWeight {
        bias: weight
            .bias
            .checked_sub(
                weight
                    .slope
                    .checked_mul(Uint128::from(dt))
                    .unwrap_or_else(|_| Uint128::zero()),
            )
            .unwrap_or_else(|_| Uint128::zero()),
        slope: max(weight.slope - slope_change, Decimal::zero()),
    }
}
