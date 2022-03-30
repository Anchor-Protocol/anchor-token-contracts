use crate::state::{Point, HISTORY, LAST_SLOPE_CHANGE, LOCKED};
use crate::utils::{
    calc_coefficient, calc_voting_power, cancel_scheduled_slope, fetch_last_checkpoint,
    fetch_slope_changes, get_period, schedule_slope_change,
};
use cosmwasm_std::{Addr, Decimal, DepsMut, Env, StdError, StdResult, Uint128};
use cw_storage_plus::U64Key;

/// ## Description
/// Checkpoint user's voting power for the current block period.
/// The function fetches last available checkpoint, calculates user's current voting power,
/// applies slope changes based on add_amount and new_end parameters,
/// schedules slope changes for total voting power
/// and saves new checkpoint for current period in [`HISTORY`] by user's address key.
/// If a user already has checkpoint for the current period then
/// this function uses it as a latest available checkpoint.
/// The function returns Ok(()) in case of success or [`StdError`]
/// in case of serialization/deserialization error.
pub(crate) fn checkpoint(
    mut deps: DepsMut,
    env: Env,
    addr: Addr,
    add_amount: Option<Uint128>,
    new_end: Option<u64>,
) -> StdResult<()> {
    let cur_period = get_period(env.block.time.seconds());
    let cur_period_key = U64Key::new(cur_period);
    let add_amount = add_amount.unwrap_or_default();
    let mut old_slope = Decimal::zero();
    let mut add_voting_power = Uint128::zero();

    // get last checkpoint
    let last_checkpoint = fetch_last_checkpoint(deps.as_ref(), &addr, &cur_period_key)?;
    let new_point = if let Some((_, point)) = last_checkpoint {
        let end = new_end.unwrap_or(point.end);
        let dt = end.saturating_sub(cur_period);
        let current_power = calc_voting_power(&point, cur_period);
        let new_slope = if dt != 0 {
            if end > point.end && add_amount.is_zero() {
                // this is extend_lock_time. Recalculating user's VP
                let mut lock = LOCKED.load(deps.storage, addr.clone())?;
                let new_voting_power = lock.amount * calc_coefficient(dt);
                // new_voting_power should be always >= current_power. saturating_sub just in case
                add_voting_power = new_voting_power.saturating_sub(current_power);
                lock.last_extend_lock_period = cur_period;
                LOCKED.save(deps.storage, addr.clone(), &lock)?;
                Decimal::from_ratio(new_voting_power, dt)
            } else {
                // this is increase lock's amount or lock creation after withdrawal
                add_voting_power = add_amount * calc_coefficient(dt);
                Decimal::from_ratio(current_power + add_voting_power, dt)
            }
        } else {
            Decimal::zero()
        };

        // cancel previously scheduled slope change
        cancel_scheduled_slope(deps.branch(), point.slope, point.end)?;

        // we need to subtract it from total VP slope
        old_slope = point.slope;

        Point {
            power: current_power + add_voting_power,
            slope: new_slope,
            start: cur_period,
            end,
        }
    } else {
        // this error can't happen since this if-branch is intended for checkpoint creation
        let end =
            new_end.ok_or_else(|| StdError::generic_err("Checkpoint initialization error"))?;
        let dt = end - cur_period;
        add_voting_power = add_amount * calc_coefficient(dt);
        let slope = Decimal::from_ratio(add_voting_power, dt);
        Point {
            power: add_voting_power,
            slope,
            start: cur_period,
            end,
        }
    };

    // schedule slope change
    schedule_slope_change(deps.branch(), new_point.slope, new_point.end)?;

    HISTORY.save(deps.storage, (addr, cur_period_key), &new_point)?;
    checkpoint_total(
        deps,
        env,
        Some(add_voting_power),
        None,
        old_slope,
        new_point.slope,
    )
}

/// ## Description
/// Checkpoint total voting power for the current block period.
/// The function fetches last available checkpoint, recalculates passed periods before the current period,
/// applies slope changes, saves all recalculated periods in [`HISTORY`] by contract address key.
/// The function returns Ok(()) in case of success or [`StdError`]
/// in case of serialization/deserialization error.
pub(crate) fn checkpoint_total(
    deps: DepsMut,
    env: Env,
    add_voting_power: Option<Uint128>,
    reduce_power: Option<Uint128>,
    old_slope: Decimal,
    new_slope: Decimal,
) -> StdResult<()> {
    let cur_period = get_period(env.block.time.seconds());
    let cur_period_key = U64Key::new(cur_period);
    let contract_addr = env.contract.address;
    let add_voting_power = add_voting_power.unwrap_or_default();

    // get last checkpoint
    let last_checkpoint = fetch_last_checkpoint(deps.as_ref(), &contract_addr, &cur_period_key)?;
    let new_point = if let Some((_, mut point)) = last_checkpoint {
        let last_slope_change = LAST_SLOPE_CHANGE
            .may_load(deps.as_ref().storage)?
            .unwrap_or(0);
        if last_slope_change < cur_period {
            let scheduled_slope_changes =
                fetch_slope_changes(deps.as_ref(), last_slope_change, cur_period)?;
            // recalculating passed points
            for (recalc_period, scheduled_change) in scheduled_slope_changes {
                point = Point {
                    power: calc_voting_power(&point, recalc_period),
                    start: recalc_period,
                    slope: point.slope - scheduled_change,
                    ..point
                };
                HISTORY.save(
                    deps.storage,
                    (contract_addr.clone(), U64Key::new(recalc_period)),
                    &point,
                )?
            }

            LAST_SLOPE_CHANGE.save(deps.storage, &cur_period)?
        }

        let new_power = (calc_voting_power(&point, cur_period) + add_voting_power)
            .saturating_sub(reduce_power.unwrap_or_default());

        Point {
            power: new_power,
            slope: point.slope - old_slope + new_slope,
            start: cur_period,
            ..point
        }
    } else {
        Point {
            power: add_voting_power,
            slope: new_slope,
            start: cur_period,
            end: 0, // we don't use 'end' in total VP calculations
        }
    };
    HISTORY.save(deps.storage, (contract_addr, cur_period_key), &new_point)
}
