#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{
    Cw20ExecuteMsg, Cw20ReceiveMsg, Logo, LogoInfo, MarketingInfoResponse, TokenInfoResponse,
};
use cw20_base::contract::{
    execute_update_marketing, execute_upload_logo, query_download_logo, query_marketing_info,
};
use cw20_base::state::{MinterData, TokenInfo, LOGO, MARKETING_INFO, TOKEN_INFO};
use cw_storage_plus::U64Key;

use crate::checkpoint::checkpoint;
use crate::error::ContractError;
use crate::state::{Config, Lock, Point, CONFIG, HISTORY, LOCKED};
use crate::utils::{
    addr_validate_to_lower, anc_token_check, calc_coefficient, calc_voting_power,
    fetch_last_checkpoint, fetch_slope_changes, get_period, time_limits_check, WEEK,
};
use anchor_token::voting_escrow::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, LockInfoResponse, QueryMsg,
    UserSlopeResponse, UserUnlockPeriodResponse, VotingPowerResponse,
};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "anchor-voting-escrow";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// ## Description
/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
/// Returns the default object of type [`Response`] if the operation was successful,
/// or a [`ContractError`] if the contract was not created.
/// ## Params
/// * **deps** is the object of type [`DepsMut`].
///
/// * **env** is the object of type [`Env`].
///
/// * **_info** is the object of type [`MessageInfo`].
///
/// * **msg** is a message of type [`InstantiateMsg`] which contains the basic settings for creating a contract
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        owner: deps.api.addr_canonicalize(&msg.owner)?,
        anchor_token: deps.api.addr_canonicalize(&msg.anchor_token)?,
    };
    CONFIG.save(deps.storage, &config)?;

    let cur_period = get_period(env.block.time.seconds());
    let point = Point {
        power: Uint128::zero(),
        start: cur_period,
        end: 0,
        slope: Decimal::zero(),
    };
    HISTORY.save(
        deps.storage,
        (env.contract.address.clone(), U64Key::new(cur_period)),
        &point,
    )?;

    set_marketing_info(&mut deps, msg)?;
    set_token_info(&mut deps, env)?;

    Ok(Response::default())
}

/// ## Description
/// Parses execute message and route it to intended function. Returns [`Response`] if execution succeed
/// or [`ContractError`] if error occurred.
///
/// ## Execute messages
/// * **ExecuteMsg::ExtendLockTime { time }** increase current lock time
///
/// * **ExecuteMsg::Receive(msg)** parse incoming message from the ANC token.
/// msg should have [`Cw20ReceiveMsg`] type.
///
/// * **ExecuteMsg::Withdraw {}** withdraw whole amount from the current lock if it has expired
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ExtendLockTime { time } => extend_lock_time(deps, env, info, time),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Withdraw {} => withdraw(deps, env, info),
        ExecuteMsg::UpdateMarketing {
            project,
            description,
            marketing,
        } => execute_update_marketing(deps, env, info, project, description, marketing)
            .map_err(|e| e.into()),
        ExecuteMsg::UploadLogo(logo) => {
            execute_upload_logo(deps, env, info, logo).map_err(|e| e.into())
        }
    }
}

/// ## Description
/// Receives a message of type [`Cw20ReceiveMsg`] and processes it depending on the received template.
/// If the template is not found in the received message, then an [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    anc_token_check(deps.as_ref(), info.sender)?;
    let sender = addr_validate_to_lower(deps.api, &cw20_msg.sender)?;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::CreateLock { time } => create_lock(deps, env, sender, cw20_msg.amount, time),
        Cw20HookMsg::ExtendLockAmount {} => deposit_for(deps, env, cw20_msg.amount, sender),
        Cw20HookMsg::DepositFor { user } => {
            let addr = addr_validate_to_lower(deps.api, &user)?;
            deposit_for(deps, env, cw20_msg.amount, addr)
        }
    }
}

/// ## Description
/// Creates a lock for the user for specified time. The time value is in seconds.
/// Evaluates that the time is within [`WEEK`]..[`MAX_LOCK_TIME`] limits.
/// Creates lock if it doesn't exist and triggers [`checkpoint`].
/// If lock is already exists, then an [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
fn create_lock(
    deps: DepsMut,
    env: Env,
    user: Addr,
    amount: Uint128,
    time: u64,
) -> Result<Response, ContractError> {
    time_limits_check(time)?;

    let block_period = get_period(env.block.time.seconds());
    let end = block_period + get_period(time);

    LOCKED.update(deps.storage, user.clone(), |lock_opt| {
        if lock_opt.is_some() && !lock_opt.unwrap().amount.is_zero() {
            return Err(ContractError::LockAlreadyExists {});
        }
        Ok(Lock {
            amount,
            start: block_period,
            end,
            last_extend_lock_period: block_period,
        })
    })?;

    checkpoint(deps, env, user, Some(amount), Some(end))?;

    Ok(Response::default().add_attribute("action", "create_lock"))
}

/// ## Description
/// Deposits 'amount' tokens to 'user' lock.
/// Triggers [`checkpoint`].
/// If lock is already expired, then an [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
fn deposit_for(
    deps: DepsMut,
    env: Env,
    amount: Uint128,
    user: Addr,
) -> Result<Response, ContractError> {
    LOCKED.update(deps.storage, user.clone(), |lock_opt| match lock_opt {
        Some(mut lock) if !lock.amount.is_zero() => {
            if lock.end <= get_period(env.block.time.seconds()) {
                Err(ContractError::LockExpired {})
            } else {
                lock.amount += amount;
                Ok(lock)
            }
        }
        _ => Err(ContractError::LockDoesntExist {}),
    })?;
    checkpoint(deps, env, user, Some(amount), None)?;

    Ok(Response::default().add_attribute("action", "deposit_for"))
}

/// ## Description
/// Withdraws whole amount of locked ANC.
/// If lock doesn't exist or it has not yet expired, then an [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
fn withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let sender = info.sender;
    // 'LockDoesntExist' is either a lock does not exist in LOCKED or a lock exits but lock.amount == 0
    let mut lock = LOCKED
        .may_load(deps.storage, sender.clone())?
        .filter(|lock| !lock.amount.is_zero())
        .ok_or(ContractError::LockDoesntExist {})?;

    let cur_period = get_period(env.block.time.seconds());
    if lock.end > cur_period {
        Err(ContractError::LockHasNotExpired {})
    } else {
        let config = CONFIG.load(deps.storage)?;
        let transfer_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.anchor_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: sender.to_string(),
                amount: lock.amount,
            })?,
            funds: vec![],
        });
        lock.amount = Uint128::zero();
        LOCKED.save(deps.storage, sender.clone(), &lock)?;

        // we need to set point to eliminate the slope influence on a future lock
        HISTORY.save(
            deps.storage,
            (sender, U64Key::new(cur_period)),
            &Point {
                power: Uint128::zero(),
                start: cur_period,
                end: cur_period,
                slope: Decimal::zero(),
            },
        )?;

        Ok(Response::default()
            .add_message(transfer_msg)
            .add_attribute("action", "withdraw"))
    }
}

/// ## Description
/// Increases current lock time by specified time. The time value is in seconds.
/// Evaluates that the time is within [`WEEK`]..[`MAX_LOCK_TIME`] limits
/// and triggers [`checkpoint`].
/// If lock doesn't exist or it expired, then an [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
/// ## Note
/// The time is added to lock's end.
/// For example, at the period 0 user locked ANC for 3 weeks.
/// In 1 week he increases time by 10 weeks thus unlock period becomes 13.
fn extend_lock_time(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    time: u64,
) -> Result<Response, ContractError> {
    let user = info.sender;
    let mut lock = LOCKED
        .may_load(deps.storage, user.clone())?
        .filter(|lock| !lock.amount.is_zero())
        .ok_or(ContractError::LockDoesntExist {})?;

    // disabling ability to extend lock time by less than a week
    time_limits_check(time)?;

    if lock.end <= get_period(env.block.time.seconds()) {
        return Err(ContractError::LockExpired {});
    };

    // should not exceed MAX_LOCK_TIME
    time_limits_check(lock.end * WEEK + time - env.block.time.seconds())?;
    lock.end += get_period(time);
    LOCKED.save(deps.storage, user.clone(), &lock)?;

    checkpoint(deps, env, user, None, Some(lock.end))?;

    Ok(Response::default().add_attribute("action", "extend_lock_time"))
}

/// # Description
/// Describes all query messages.
/// ## Queries
/// * **QueryMsg::TotalVotingPower {}** total voting power at current block
/// * **QueryMsg::UserVotingPower { user }** user's voting power at current block
/// * **QueryMsg::TotalVotingPowerAt { time }** total voting power at specified time
/// * **QueryMsg::TotalVotingPowerAtPeriod { period }** total voting power at specified period
/// * **QueryMsg::UserVotingPowerAt { time }** user's voting power at specified time
/// * **QueryMsg::UserVotingPowerAtPeriod { period }** user's voting power at specified period
/// * **QueryMsg::GetLastUserSlope { user }** user's most recently recorded slope
/// * **QueryMsg::GetUserUnlockTime { user }** user's lock end time
/// * **QueryMsg::LockInfo { user }** user's lock information
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::TotalVotingPower {} => to_binary(&get_total_voting_power(deps, env, None)?),
        QueryMsg::UserVotingPower { user } => {
            to_binary(&get_user_voting_power(deps, env, user, None)?)
        }
        QueryMsg::TotalVotingPowerAt { time } => {
            to_binary(&get_total_voting_power(deps, env, Some(time))?)
        }
        QueryMsg::TotalVotingPowerAtPeriod { period } => {
            to_binary(&get_total_voting_power_at_period(deps, env, period)?)
        }
        QueryMsg::UserVotingPowerAt { user, time } => {
            to_binary(&get_user_voting_power(deps, env, user, Some(time))?)
        }
        QueryMsg::UserVotingPowerAtPeriod { user, period } => {
            to_binary(&get_user_voting_power_at_period(deps, user, period)?)
        }
        QueryMsg::GetLastUserSlope { user } => to_binary(&get_last_user_slope(deps, env, user)?),
        QueryMsg::GetUserUnlockPeriod { user } => to_binary(&get_user_unlock_time(deps, user)?),
        QueryMsg::LockInfo { user } => to_binary(&get_user_lock_info(deps, user)?),
        QueryMsg::Config {} => {
            let config = CONFIG.load(deps.storage)?;
            to_binary(&ConfigResponse {
                owner: deps.api.addr_humanize(&config.owner)?.to_string(),
                anchor_token: deps.api.addr_humanize(&config.anchor_token)?.to_string(),
            })
        }
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps, env)?),
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
    }
}

/// # Description
/// Calculates total voting power at the given time.
/// If time is None then calculates voting power at the current block period.
fn get_total_voting_power(
    deps: Deps,
    env: Env,
    time: Option<u64>,
) -> StdResult<VotingPowerResponse> {
    let period = get_period(time.unwrap_or_else(|| env.block.time.seconds()));
    get_total_voting_power_at_period(deps, env, period)
}

/// # Description
/// Calculates user's voting power at the given time.
/// If time is None then calculates voting power at the current block period.
fn get_user_voting_power(
    deps: Deps,
    env: Env,
    user: String,
    time: Option<u64>,
) -> StdResult<VotingPowerResponse> {
    let period = get_period(time.unwrap_or_else(|| env.block.time.seconds()));
    get_user_voting_power_at_period(deps, user, period)
}

/// # Description
/// Calculates a user's voting power at a given period number.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **user** is an object of type String. This is the user/staker for which we fetch the current voting power (veANC balance).
///
/// * **period** is [`u64`]. This is the period number at which to fetch the user's voting power (veANC balance).
fn get_user_voting_power_at_period(
    deps: Deps,
    user: String,
    period: u64,
) -> StdResult<VotingPowerResponse> {
    let user = addr_validate_to_lower(deps.api, &user)?;
    let period_key = U64Key::new(period);

    let last_checkpoint = fetch_last_checkpoint(deps, &user, &period_key)?;

    if let Some(point) = last_checkpoint.map(|(_, point)| point) {
        // the point right in this period was found
        let voting_power = if point.start == period {
            point.power
        } else {
            // the point before this period was found thus we can calculate VP in the period
            // we are interested in
            calc_voting_power(&point, period)
        };
        Ok(VotingPowerResponse { voting_power })
    } else {
        // user not found
        Ok(VotingPowerResponse {
            voting_power: Uint128::zero(),
        })
    }
}

/// # Description
/// Calculates the total voting power (total veANC supply) at the given period number.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **env** is an object of type [`Env`].
///
/// * **period** is [`u64`]. This is the period number at which we fetch the total voting power (veANC supply).
fn get_total_voting_power_at_period(
    deps: Deps,
    env: Env,
    period: u64,
) -> StdResult<VotingPowerResponse> {
    let period_key = U64Key::new(period);

    let last_checkpoint = fetch_last_checkpoint(deps, &env.contract.address, &period_key)?;

    let point = last_checkpoint.map_or(
        Point {
            power: Uint128::zero(),
            start: period,
            end: period,
            slope: Decimal::zero(),
        },
        |(_, point)| point,
    );

    let voting_power = if point.start == period {
        point.power
    } else {
        let scheduled_slope_changes = fetch_slope_changes(deps, point.start, period)?;
        let mut init_point = point;
        for (recalc_period, scheduled_change) in scheduled_slope_changes {
            init_point = Point {
                power: calc_voting_power(&init_point, recalc_period),
                start: recalc_period,
                slope: init_point.slope - scheduled_change,
                ..init_point
            }
        }
        calc_voting_power(&init_point, period)
    };

    Ok(VotingPowerResponse { voting_power })
}

/// # Description
/// Returns user's most recently recorded rate of voting power decrease.
fn get_last_user_slope(deps: Deps, env: Env, user: String) -> StdResult<UserSlopeResponse> {
    let user = addr_validate_to_lower(deps.api, &user)?;
    let period = get_period(env.block.time.seconds());
    let period_key = U64Key::new(period);
    let last_checkpoint = fetch_last_checkpoint(deps, &user, &period_key)?;

    let slope = if let Some((_, point)) = last_checkpoint {
        Uint128::new(1u128) * point.slope
    } else {
        Uint128::zero()
    };

    Ok(UserSlopeResponse { slope })
}

/// # Description
/// Returns user's lock `end` time, which is the period when the lock expires.
fn get_user_unlock_time(deps: Deps, user: String) -> StdResult<UserUnlockPeriodResponse> {
    let addr = addr_validate_to_lower(deps.api, &user)?;
    if let Some(lock) = LOCKED.may_load(deps.storage, addr)? {
        Ok(UserUnlockPeriodResponse {
            unlock_period: lock.end,
        })
    } else {
        Err(StdError::generic_err("User lock not found"))
    }
}

/// # Description
/// Returns user's lock information in [`LockInfoResponse`] type.
fn get_user_lock_info(deps: Deps, user: String) -> StdResult<LockInfoResponse> {
    let addr = addr_validate_to_lower(deps.api, &user)?;
    if let Some(lock) = LOCKED.may_load(deps.storage, addr)? {
        let resp = LockInfoResponse {
            amount: lock.amount,
            coefficient: calc_coefficient(lock.end - lock.last_extend_lock_period),
            start: lock.start,
            end: lock.end,
        };
        Ok(resp)
    } else {
        Err(StdError::generic_err("User lock not found"))
    }
}

/// # Description
/// Fetch the veANC token information, such as the token name, symbol, decimals and total supply (total voting power).
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **env** is an object of type [`Env`].
fn query_token_info(deps: Deps, env: Env) -> StdResult<TokenInfoResponse> {
    let info = TOKEN_INFO.load(deps.storage)?;
    let total_vp = get_total_voting_power(deps, env, None)?;
    let res = TokenInfoResponse {
        name: info.name,
        symbol: info.symbol,
        decimals: info.decimals,
        total_supply: total_vp.voting_power,
    };
    Ok(res)
}

fn set_marketing_info(deps: &mut DepsMut<'_>, msg: InstantiateMsg) -> StdResult<()> {
    // Store Marketing info
    let marketing_info = if let Some(marketing) = msg.marketing {
        let logo = if let Some(logo) = marketing.logo {
            LOGO.save(deps.storage, &logo)?;
            match logo {
                Logo::Url(url) => Some(LogoInfo::Url(url)),
                Logo::Embedded(_) => Some(LogoInfo::Embedded),
            }
        } else {
            None
        };
        MarketingInfoResponse {
            project: marketing.project,
            description: marketing.description,
            marketing: marketing
                .marketing
                .map(|addr| addr_validate_to_lower(deps.api, &addr))
                .transpose()?,
            logo,
        }
    } else {
        // adding default marketing info so that `owner` can later update
        MarketingInfoResponse {
            project: None,
            description: None,
            marketing: Some(deps.api.addr_validate(&msg.owner)?),
            logo: None,
        }
    };

    MARKETING_INFO.save(deps.storage, &marketing_info)?;
    Ok(())
}

fn set_token_info(deps: &mut DepsMut<'_>, env: Env) -> StdResult<()> {
    // Store token info
    let data = TokenInfo {
        name: "veANC".to_string(),
        symbol: "veANC".to_string(),
        decimals: 6,
        total_supply: Uint128::zero(),
        mint: Some(MinterData {
            minter: env.contract.address,
            cap: None,
        }),
    };

    TOKEN_INFO.save(deps.storage, &data)?;
    Ok(())
}