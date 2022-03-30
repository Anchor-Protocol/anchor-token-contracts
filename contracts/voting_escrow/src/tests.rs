use crate::checkpoint::{checkpoint, checkpoint_total};
use crate::contract::{execute, instantiate, query};
use crate::error::ContractError::{
    Cw20Base, InsufficientStaked, LockDoesntExist, LockExpired, LockHasNotExpired,
    LockTimeLimitsError, Unauthorized,
};
use crate::state::{Config, Lock, Point, HISTORY, LAST_SLOPE_CHANGE, SLOPE_CHANGES};
use crate::utils::{
    calc_voting_power, cancel_scheduled_slope, fetch_last_checkpoint, schedule_slope_change,
    MAX_LOCK_TIME, MIN_LOCK_TIME, WEEK,
};
use anchor_token::voting_escrow::{
    ConfigResponse, ExecuteMsg, InstantiateMarketingInfo, InstantiateMsg, LockInfoResponse,
    QueryMsg, UserSlopeResponse, UserUnlockPeriodResponse, VotingPowerResponse,
};
use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{
    from_binary, Addr, Binary, CanonicalAddr, Decimal, MessageInfo, OwnedDeps, Timestamp, Uint128,
};
use cw20::{
    DownloadLogoResponse, EmbeddedLogo, Logo, LogoInfo, MarketingInfoResponse, TokenInfoResponse,
};
use cw20_base::ContractError as Cw20BaseContractError;
use cw_storage_plus::U64Key;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let config = Config {
        owner: CanonicalAddr::from("owner".as_bytes()),
        anchor_token: CanonicalAddr::from("anchor".as_bytes()),
    };

    let msg = InstantiateMsg {
        owner: String::from_utf8_lossy(config.owner.as_slice()).to_string(),
        anchor_token: String::from_utf8_lossy(config.anchor_token.as_slice()).to_string(),
        marketing: Some(InstantiateMarketingInfo {
            project: Some("voted-escrow".to_string()),
            description: Some("voted-escrow".to_string()),
            logo: Some(Logo::Url("votes-escrow-url".to_string())),
            marketing: Some("marketing".to_string()),
        }),
    };

    let info = mock_info("owner", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();

    assert_eq!(config.owner, "owner".to_string());
    assert_eq!(config.anchor_token, "anchor".to_string());

    let res = query(deps.as_ref(), mock_env(), QueryMsg::MarketingInfo {}).unwrap();
    let marketing: MarketingInfoResponse = from_binary(&res).unwrap();

    assert_eq!(marketing.project.unwrap(), "voted-escrow".to_string());
    assert_eq!(marketing.description.unwrap(), "voted-escrow".to_string());
    assert_eq!(
        marketing.logo.unwrap(),
        LogoInfo::Url("votes-escrow-url".to_string())
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();

    assert_eq!(token_info.name, "veANC".to_string());
    assert_eq!(token_info.symbol, "veANC".to_string());
    assert_eq!(token_info.decimals, 6);
    assert_eq!(token_info.total_supply, Uint128::zero());
}

#[test]
fn test_create_lock() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor".to_string(),
        marketing: None,
    };

    let info = mock_info("owner", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::ExtendLockTime {
        user: "addr0000".to_string(),
        time: WEEK,
    };

    // only anchor token is authorized to create locks
    let info = mock_info("random", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(Unauthorized {}) => {}
        _ => panic!("Must return Unauthorized error"),
    }

    let info = mock_info("owner", &[]);

    let msg = ExecuteMsg::ExtendLockTime {
        user: "addr0000".to_string(),
        time: 2 * 86400,
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
    match res {
        Err(LockTimeLimitsError {}) => {}
        _ => panic!("Must return LockTimeLimitsError error"),
    }

    // time provided is above limit
    let msg = ExecuteMsg::ExtendLockTime {
        user: "addr0000".to_string(),
        time: MAX_LOCK_TIME + 86400,
    };

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
    match res {
        Err(LockTimeLimitsError {}) => {}
        _ => panic!("Must return LockTimeLimitsError error"),
    }

    // creates lock successfully
    let msg = ExecuteMsg::ExtendLockTime {
        user: "addr0000".to_string(),
        time: MIN_LOCK_TIME,
    };

    let env = mock_env();
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "extend_lock_time");

    let msg = ExecuteMsg::ExtendLockAmount {
        user: "addr0000".to_string(),
        amount: Uint128::from(10u128),
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "extend_lock_amount");

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::LockInfo {
            user: "addr0000".to_string(),
        },
    )
    .unwrap();

    let lock_info: LockInfoResponse = from_binary(&res).unwrap();

    let max_period = MAX_LOCK_TIME / WEEK;
    let lock_period = MIN_LOCK_TIME / WEEK + 1;
    let expected_coeff = Decimal::from_ratio(25 * lock_period, max_period * 10);

    let start_period = env.block.time.seconds() / WEEK;

    let expected_lock = Lock {
        amount: Uint128::from(10u128),
        start: start_period,
        end: start_period + lock_period,
        last_extend_lock_period: 0u64,
    };

    assert_eq!(lock_info.amount, expected_lock.amount);
    assert_eq!(lock_info.start, expected_lock.start);
    assert_eq!(lock_info.end, expected_lock.end);
    assert_eq!(lock_info.coefficient, expected_coeff);

    // user voting power at `start` should be AMOUNT * coefficient
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::UserVotingPowerAtPeriod {
            user: "addr0000".to_string(),
            period: lock_info.start,
        },
    )
    .unwrap();
    let voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    assert_eq!(
        voting_power.voting_power,
        Uint128::from(10u64) * expected_coeff
    );

    // user voting power at `end` should be 0
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::UserVotingPowerAtPeriod {
            user: "addr0000".to_string(),
            period: lock_info.end,
        },
    )
    .unwrap();
    let voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    assert_eq!(voting_power.voting_power, Uint128::zero());
}

#[test]
fn test_extend_lock_amount() {
    let (mut deps, _, owner_info) =
        init_lock_factory("addr0000".to_string(), Some(Uint128::from(20u64)), None);

    let msg = ExecuteMsg::ExtendLockAmount {
        user: "addr0000".to_string(),
        amount: Uint128::from(10u128),
    };

    // only anchor token is authorized to extend lock amount
    let info = mock_info("random", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(Unauthorized {}) => {}
        _ => panic!("Must return Unauthorized error"),
    };

    // cannot extend lock amount for a user w/o a lock
    let msg = ExecuteMsg::ExtendLockAmount {
        user: "random0000".to_string(),
        amount: Uint128::from(10u128),
    };

    let res = execute(deps.as_mut(), mock_env(), owner_info.clone(), msg);
    match res {
        Err(LockDoesntExist {}) => {}
        _ => panic!("Must return LockDoesntExist error"),
    };

    // cannot extend lock amount for an expired lock
    let msg = ExecuteMsg::ExtendLockAmount {
        user: "addr0000".to_string(),
        amount: Uint128::from(10u128),
    };

    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(env.block.time.seconds() + MIN_LOCK_TIME + WEEK);
    let res = execute(deps.as_mut(), env, owner_info.clone(), msg.clone());
    match res {
        Err(LockExpired {}) => {}
        _ => panic!("Must return LockExpired error"),
    };

    // extends lock amount successfully
    let res = execute(deps.as_mut(), mock_env(), owner_info, msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "extend_lock_amount");

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::LockInfo {
            user: "addr0000".to_string(),
        },
    )
    .unwrap();
    let lock_info: LockInfoResponse = from_binary(&res).unwrap();

    assert_eq!(lock_info.amount, Uint128::from(30u64));
}

#[test]
fn test_extend_lock_time() {
    let (mut deps, _, _) = init_lock_factory("addr0000".to_string(), None, Some(MIN_LOCK_TIME));
    let info = mock_info("owner", &[]);

    // cannot extend lock time beyond MAX_LOCK_TIME
    let four_years = WEEK * 365 * 4;
    let msg = ExecuteMsg::ExtendLockTime {
        user: "addr0000".to_string(),
        time: four_years,
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
    match res {
        Err(LockTimeLimitsError {}) => {}
        _ => panic!("Must return LockTimeLimitsError error"),
    };

    let curr_lock_info: LockInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::LockInfo {
                user: "addr0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();

    // extends lock time successfully
    let msg = ExecuteMsg::ExtendLockTime {
        user: "addr0000".to_string(),
        time: WEEK * 3,
    };
    let env = mock_env();
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "extend_lock_time");

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::LockInfo {
            user: "addr0000".to_string(),
        },
    )
    .unwrap();
    let updated_lock_info: LockInfoResponse = from_binary(&res).unwrap();

    // checks `end` time was extended by 3 weeks
    assert_eq!(updated_lock_info.end, curr_lock_info.end + 3);
}

#[test]
fn test_withdraw() {
    let (mut deps, _, _) = init_lock_factory(
        "addr0000".to_string(),
        Some(Uint128::from(100u64)),
        Some(MIN_LOCK_TIME * 2),
    );

    let info = mock_info("owner", &[]);

    let msg = ExecuteMsg::Withdraw {
        user: "random0000".to_string(),
        amount: Uint128::from(100u64),
    };

    // cannot withdraw for a user w/o a lock
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
    match res {
        Err(LockDoesntExist {}) => {}
        _ => panic!("Must return LockDoesntExist error"),
    };

    let msg = ExecuteMsg::Withdraw {
        user: "addr0000".to_string(),
        amount: Uint128::from(100u64),
    };

    // cannot withdraw if lock has not expired
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone());
    match res {
        Err(LockHasNotExpired {}) => {}
        _ => panic!("Must return LockHasNotExpired error"),
    };

    // withdraw successfully
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(env.block.time.seconds() + MIN_LOCK_TIME * 2 + WEEK);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "withdraw");

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::UserVotingPower {
            user: "addr0000".to_string(),
        },
    )
    .unwrap();
    let user_voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    assert_eq!(user_voting_power.voting_power, Uint128::zero());

    let res = query(deps.as_ref(), env.clone(), QueryMsg::TotalVotingPower {}).unwrap();
    let total_voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    assert_eq!(total_voting_power.voting_power, Uint128::zero());

    // cannot withdraw if user has zero amount `locked`
    let curr_lock_info: LockInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::LockInfo {
                user: "addr0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(curr_lock_info.amount, Uint128::from(0u64));

    let res = execute(deps.as_mut(), env, info, msg);
    match res {
        Err(InsufficientStaked {}) => {}
        _ => panic!("Must return InsufficientStaked error"),
    };
}

#[test]
fn test_update_marketing() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor".to_string(),
        marketing: None,
    };

    let owner_info = mock_info("owner", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    let msg = ExecuteMsg::UpdateMarketing {
        project: Some("voting-escrow".to_string()),
        description: Some("voting-escrow".to_string()),
        marketing: Some("marketingaddr0000".to_string()),
    };

    // contract `owner` can update marketing info when no `marketing` owner is set
    let res = execute(deps.as_mut(), mock_env(), owner_info, msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_marketing");

    let res = query(deps.as_ref(), mock_env(), QueryMsg::MarketingInfo {}).unwrap();
    let marketing_info: MarketingInfoResponse = from_binary(&res).unwrap();

    assert_eq!(
        marketing_info.description.unwrap(),
        "voting-escrow".to_string()
    );
    assert_eq!(marketing_info.project.unwrap(), "voting-escrow".to_string());
    assert_eq!(
        marketing_info.marketing.unwrap(),
        "marketingaddr0000".to_string()
    );

    // only `marketing` owner can make subsequent updates
    let msg = ExecuteMsg::UpdateMarketing {
        project: None,
        description: None,
        marketing: None,
    };
    let info = mock_info("random", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(Cw20Base(Cw20BaseContractError::Unauthorized {})) => {}
        _ => panic!("Must return Unauthorized error"),
    }
}

#[test]
fn test_upload_logo() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor".to_string(),
        marketing: None,
    };

    let owner_info = mock_info("owner", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    // only `marketing` owner can update logo
    let info = mock_info("random", &[]);
    let msg = ExecuteMsg::UploadLogo(Logo::Url("cool-logo".to_string()));
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(Cw20Base(Cw20BaseContractError::Unauthorized {})) => {}
        _ => panic!("Must return Unauthorized error"),
    }

    // upload logo successfully
    let png_logo = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let msg = ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Png(Binary::from(&png_logo))));
    let res = execute(deps.as_mut(), mock_env(), owner_info, msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "upload_logo");

    let msg = QueryMsg::MarketingInfo {};
    let res = query(deps.as_ref(), mock_env(), msg).unwrap();
    let marketing_info: MarketingInfoResponse = from_binary(&res).unwrap();

    assert_ne!(marketing_info.logo, None);

    let msg = QueryMsg::DownloadLogo {};
    let res = query(deps.as_ref(), mock_env(), msg).unwrap();
    let logo: DownloadLogoResponse = from_binary(&res).unwrap();

    assert_eq!(logo.mime_type, "image/png".to_string());
    assert_eq!(logo.data, Binary::from(&png_logo));
}

#[test]
fn test_get_total_voting_power() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor".to_string(),
        marketing: None,
    };

    let owner_info = mock_info("owner", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    let users_to_create_lock_for = vec![
        ("user1".to_string(), Uint128::from(100u64), MIN_LOCK_TIME),
        ("user2".to_string(), Uint128::from(50u64), 2 * MIN_LOCK_TIME),
    ];

    let env = mock_env();
    // create user locks
    for (user, lock_amount, lock_time) in users_to_create_lock_for {
        let msg = ExecuteMsg::ExtendLockTime {
            user: user.clone(),
            time: lock_time,
        };
        let _res = execute(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();
        let msg = ExecuteMsg::ExtendLockAmount {
            user,
            amount: lock_amount,
        };
        let _res = execute(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();
    }

    // voting power at start time should include both user1 and user2
    let res = query(deps.as_ref(), env.clone(), QueryMsg::TotalVotingPower {}).unwrap();
    let total_voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    let max_period = MAX_LOCK_TIME / WEEK;
    let lock_period1 = MIN_LOCK_TIME / WEEK + 1;
    let lock_period2 = 2 * MIN_LOCK_TIME / WEEK + 1;
    let user1_coeff = Decimal::from_ratio(25 * lock_period1, max_period * 10); // (2.5 * lock_period1)/max_period
    let user2_coeff = Decimal::from_ratio(25 * lock_period2, max_period * 10); // (2.5 * lock_period2)/max_period

    let user1_voting_power = Uint128::from(100u64) * user1_coeff; // lock_amount * coeff1
    let user2_voting_power = Uint128::from(50u64) * user2_coeff; // lock_amount * coeff2

    let expected_total_voting_power = user1_voting_power + user2_voting_power;

    assert_eq!(total_voting_power.voting_power, expected_total_voting_power);

    let start_time = env.block.time.seconds();

    // voting power after 2 weeks should only include user2
    let one_year_later = start_time + (MIN_LOCK_TIME + WEEK * 2);
    let msg = QueryMsg::TotalVotingPowerAt {
        time: one_year_later,
    };
    let res = query(deps.as_ref(), env.clone(), msg).unwrap();
    let total_voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    let user2_slope = Decimal::from_ratio(user2_voting_power, lock_period2); // voting_power / (end - start)

    // total voting power should be user2's voting power with 2 weeks reduction
    // user2_vp/total_vp = user2_vp - slope * (current_time - start_time)
    let expected_voting_power = user2_voting_power
        .checked_sub(user2_slope * (Uint128::from((one_year_later - start_time) / WEEK + 1)))
        .unwrap();

    assert_eq!(total_voting_power.voting_power, expected_voting_power);

    let one_year_later_period = one_year_later / WEEK;
    let msg = QueryMsg::TotalVotingPowerAtPeriod {
        period: one_year_later_period,
    };
    let res = query(deps.as_ref(), env, msg).unwrap();
    let total_voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    assert_eq!(total_voting_power.voting_power, expected_voting_power);
}

#[test]
fn test_get_user_voting_power() {
    let (deps, _, _) = init_lock_factory(
        "addr0000".to_string(),
        Some(Uint128::from(100u64)),
        Some(MIN_LOCK_TIME),
    );

    let env = mock_env();
    let msg = QueryMsg::UserVotingPower {
        user: "addr0000".to_string(),
    };

    // user voting power at start time
    let res = query(deps.as_ref(), env.clone(), msg).unwrap();
    let user_voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    let max_period = MAX_LOCK_TIME / WEEK;
    let lock_period = MIN_LOCK_TIME / WEEK + 1;
    let coeff = Decimal::from_ratio(Uint128::from(25 * lock_period), max_period * 10); // (2.5 * lock_time)/MAX_LOCK_TIME
    let expected_voting_power = Uint128::from(100u64) * coeff; // lock_amount * ((2.5 * lock_time)/MAX_LOCK_TIME)

    assert_eq!(user_voting_power.voting_power, expected_voting_power);

    let start_time = env.block.time.seconds();

    // user voting power 1 week later
    let one_week_later = start_time + (WEEK + 1);
    let msg = QueryMsg::UserVotingPowerAt {
        user: "addr0000".to_string(),
        time: one_week_later,
    };
    let res = query(deps.as_ref(), env.clone(), msg).unwrap();
    let user_voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    let user_slope = Decimal::from_ratio(expected_voting_power, MIN_LOCK_TIME);

    let expected_voting_power = expected_voting_power
        .checked_sub(user_slope * (Uint128::from(one_week_later - start_time)))
        .unwrap();

    assert_eq!(user_voting_power.voting_power, expected_voting_power);

    let one_week_later_period = one_week_later / WEEK;
    let msg = QueryMsg::UserVotingPowerAtPeriod {
        user: "addr0000".to_string(),
        period: one_week_later_period,
    };
    let res = query(deps.as_ref(), env, msg).unwrap();
    let user_voting_power: VotingPowerResponse = from_binary(&res).unwrap();

    assert_eq!(user_voting_power.voting_power, expected_voting_power);
}

#[test]
fn test_get_last_user_slope() {
    let (mut deps, _, _) = init_lock_factory(
        "addr0000".to_string(),
        Some(Uint128::from(100u64)),
        Some(MIN_LOCK_TIME * 2),
    );

    let env = mock_env();
    let msg = QueryMsg::LastUserSlope {
        user: "addr0000".to_string(),
    };
    let res = query(deps.as_ref(), env.clone(), msg.clone()).unwrap();
    let user_slope: UserSlopeResponse = from_binary(&res).unwrap();

    let max_period = MAX_LOCK_TIME / WEEK;
    let lock_period = MIN_LOCK_TIME * 2 / WEEK + 1;
    let user_coeff = Decimal::from_ratio(25 * lock_period, max_period * 10);
    let user_vp = Uint128::from(100u64) * user_coeff;
    let expected_slope = Decimal::from_ratio(user_vp, Uint128::from(lock_period));

    assert_eq!(user_slope.slope, expected_slope);

    // extending lock time should update the slope
    let info = mock_info("owner", &[]);
    let six_weeks = 6 * WEEK;
    let extend_lock_time_msg = ExecuteMsg::ExtendLockTime {
        user: "addr0000".to_string(),
        time: six_weeks,
    };
    let _res = execute(deps.as_mut(), env.clone(), info, extend_lock_time_msg).unwrap();

    // user voting power is updated after extend_lock_time by old_vp * new_coeff
    let user_coeff = Decimal::from_ratio(25 * (lock_period + 6), max_period * 10);
    let user_vp = user_vp * user_coeff;

    let res = query(deps.as_ref(), env, msg).unwrap();
    let user_slope: UserSlopeResponse = from_binary(&res).unwrap();

    let expected_slope =
        Uint128::new(1u128) * Decimal::from_ratio(user_vp, Uint128::from(lock_period + 6));

    assert_eq!(user_slope.slope * Uint128::new(1u128), expected_slope);
}

#[test]
fn test_get_user_unlock_period() {
    let (mut deps, _, _) = init_lock_factory(
        "addr0000".to_string(),
        Some(Uint128::from(100u64)),
        Some(MIN_LOCK_TIME * 2),
    );

    let msg = QueryMsg::UserUnlockPeriod {
        user: "addr0000".to_string(),
    };
    let env = mock_env();
    let res = query(deps.as_ref(), env.clone(), msg.clone()).unwrap();
    let user_unlock_period: UserUnlockPeriodResponse = from_binary(&res).unwrap();

    let start_time = env.block.time.seconds();
    let expected_unlock_period = (start_time + MIN_LOCK_TIME * 2) / WEEK;

    assert_eq!(user_unlock_period.unlock_period, expected_unlock_period);

    // extending lock time should update unlock period
    let info = mock_info("owner", &[]);
    let six_weeks = 6 * WEEK;
    let extend_lock_time_msg = ExecuteMsg::ExtendLockTime {
        user: "addr0000".to_string(),
        time: six_weeks,
    };
    let _res = execute(deps.as_mut(), env.clone(), info, extend_lock_time_msg).unwrap();

    let res = query(deps.as_ref(), env, msg).unwrap();
    let user_unlock_period: UserUnlockPeriodResponse = from_binary(&res).unwrap();

    let expected_unlock_period = (start_time + MIN_LOCK_TIME * 2 + 6 * WEEK) / WEEK;

    assert_eq!(user_unlock_period.unlock_period, expected_unlock_period);
}

#[test]
fn test_checkpoint() {
    let mut deps = mock_dependencies(&[]);

    let user = Addr::unchecked("addr0001".to_string());
    let mut env = mock_env();
    let start = env.block.time.seconds() / WEEK;
    let end = start + 4;
    checkpoint(
        deps.as_mut(),
        env.clone(),
        user.clone(),
        Some(Uint128::from(100u64)),
        Some(end),
    )
    .unwrap();

    let period_key = U64Key::new(end);
    let last_checkpoint = fetch_last_checkpoint(deps.as_ref(), &user, &period_key).unwrap();

    let max_period = MAX_LOCK_TIME / WEEK;
    let coeff = Decimal::from_ratio(Uint128::from(10u64), max_period); // (2.5 * 4)/208
    let expected_power = Uint128::from(100u64) * coeff;
    let expected_slope = Decimal::from_ratio(expected_power, Uint128::from(4u64));

    let expected_point = Point {
        power: expected_power,
        start,
        end,
        slope: expected_slope,
    };

    match last_checkpoint {
        Some((_, point)) => {
            assert_eq!(point.start, expected_point.start);
            assert_eq!(point.end, expected_point.end);
            assert_eq!(point.slope, expected_point.slope);
            assert_eq!(point.power, expected_point.power);
        }
        _ => panic!("Excepted a checkpoint to be found!"),
    };

    // slope should be zero for an expired lock
    env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 4 * WEEK + 1);
    checkpoint(deps.as_mut(), env.clone(), user.clone(), None, None).unwrap();

    let period_key = U64Key::new(env.block.time.seconds() / WEEK);

    let last_checkpoint = fetch_last_checkpoint(deps.as_ref(), &user, &period_key).unwrap();

    match last_checkpoint {
        Some((_, point)) => {
            assert_eq!(point.slope, Decimal::zero());
        }
        _ => panic!("Excepted a checkpoint to be found!"),
    };
}

#[test]
fn test_checkpoint_total() {
    let mut deps = mock_dependencies(&[]);

    let owner = Addr::unchecked("owner".to_string());
    let period = 2;
    let period_key = U64Key::new(period);

    let point = Point {
        power: Uint128::from(100u64),
        start: 0u64,
        end: 100u64,
        slope: Decimal::from_ratio(Uint128::from(4u64), Uint128::from(1u64)),
    };

    LAST_SLOPE_CHANGE.save(&mut deps.storage, &(0)).unwrap();

    HISTORY
        .save(&mut deps.storage, (owner.clone(), period_key), &point)
        .unwrap();

    let slope_changes_to_schedule: Vec<(u64, u64)> = vec![(2, 0), (3, 2), (4, 2)];

    for (period, slope) in slope_changes_to_schedule {
        let slope = Decimal::from_ratio(Uint128::from(slope), Uint128::from(1u64));
        SLOPE_CHANGES
            .save(&mut deps.storage, U64Key::new(period), &slope)
            .unwrap();
    }

    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 5 * WEEK);
    env.contract.address = owner.clone();
    checkpoint_total(
        deps.as_mut(),
        env,
        None,
        None,
        Decimal::zero(),
        Decimal::zero(),
    )
    .unwrap();

    // check passed points are recalculated
    let updated_slope_period_2 = HISTORY
        .load(&deps.storage, (owner.clone(), U64Key::new(2)))
        .unwrap()
        .slope
        * Uint128::from(1u64);

    let updated_slope_period_3 = HISTORY
        .load(&deps.storage, (owner.clone(), U64Key::new(3)))
        .unwrap()
        .slope
        * Uint128::from(1u64);

    let updated_slope_period_4 = HISTORY
        .load(&deps.storage, (owner, U64Key::new(4)))
        .unwrap()
        .slope
        * Uint128::from(1u64);

    assert_eq!(updated_slope_period_2, Uint128::from(4u64));
    assert_eq!(updated_slope_period_3, Uint128::from(2u64));
    assert_eq!(updated_slope_period_4, Uint128::zero());
}

#[test]
fn test_calc_voting_power_util() {
    let point = Point {
        power: Uint128::from(100u64),
        start: 0u64,
        end: 100u64,
        slope: Decimal::from_ratio(Uint128::from(99999999999999999999u128), Uint128::from(1u64)),
    };
    let period = Uint128::MAX.u128() as u64;

    // checks vp remains the same when multiplication overflows
    let voting_power = calc_voting_power(&point, period);

    assert_eq!(voting_power, point.power);

    let point = Point {
        power: Uint128::from(200u64),
        start: 0u64,
        end: 100u64,
        slope: Decimal::from_ratio(Uint128::from(5u64), Uint128::from(3u64)),
    };

    // checks vp is rounded up correctly
    let voting_power = calc_voting_power(&point, point.end);

    let expected_vp = point.power
        - Uint128::new(1u128)
            * (Decimal::from_ratio(Uint128::from(500u64), Uint128::from(3u64)) + Decimal::one());

    assert_eq!(voting_power, expected_vp);

    let point = Point {
        power: Uint128::from(200u64),
        start: 0u64,
        end: 100u64,
        slope: Decimal::from_ratio(Uint128::from(500u64), Uint128::from(3u64)),
    };

    // checks vp is zero when sub overflows
    let voting_power = calc_voting_power(&point, point.end);

    assert_eq!(voting_power, Uint128::zero());
}

#[test]
fn test_slope_changes_util() {
    let mut deps = mock_dependencies(&[]);
    let slope = Decimal::from_ratio(Uint128::from(10u64), Uint128::from(1u64));
    let period = 2;
    let period_key = U64Key::from(period);

    SLOPE_CHANGES
        .save(&mut deps.storage, period_key.clone(), &slope)
        .unwrap();

    LAST_SLOPE_CHANGE
        .save(&mut deps.storage, &(period - 1))
        .unwrap();

    // canceling scheduled slopes decreases current slope by change
    let slope_change = Decimal::from_ratio(Uint128::from(5u64), Uint128::from(1u64));
    cancel_scheduled_slope(deps.as_mut(), slope_change, period).unwrap();

    let new_slope = SLOPE_CHANGES
        .load(&deps.storage, period_key.clone())
        .unwrap();

    assert_eq!(
        new_slope,
        Decimal::from_ratio(Uint128::from(5u64), Uint128::from(1u64))
    );

    LAST_SLOPE_CHANGE
        .save(&mut deps.storage, &(period + 1))
        .unwrap();

    // canceling scheduled slopes after `LAST_SLOPE_CHANGE` does nothing
    cancel_scheduled_slope(deps.as_mut(), slope_change, period).unwrap();

    let new_slope = SLOPE_CHANGES
        .load(&deps.storage, period_key.clone())
        .unwrap();

    assert_eq!(
        new_slope,
        Decimal::from_ratio(Uint128::from(5u64), Uint128::from(1u64))
    );

    // scheduling slope changes adds change to existing slope
    schedule_slope_change(deps.as_mut(), Decimal::one(), period).unwrap();

    let new_slope = SLOPE_CHANGES.load(&deps.storage, period_key).unwrap();

    assert_eq!(
        new_slope,
        Decimal::from_ratio(Uint128::from(6u64), Uint128::from(1u64))
    );
}

fn init_lock_factory(
    user: String,
    lock_amount: Option<Uint128>,
    lock_time: Option<u64>,
) -> (
    OwnedDeps<MockStorage, MockApi, MockQuerier>,
    MessageInfo,
    MessageInfo,
) {
    let lock_amount = lock_amount.unwrap_or_else(|| Uint128::from(10u64));
    let lock_time = lock_time.unwrap_or(MIN_LOCK_TIME);

    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor".to_string(),
        marketing: None,
    };

    let owner_info = mock_info("owner", &[]);
    let anchor_info = mock_info("anchor", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ExtendLockTime {
        user: user.clone(),
        time: lock_time,
    };
    let res = execute(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "extend_lock_time");

    let msg = ExecuteMsg::ExtendLockAmount {
        user,
        amount: lock_amount,
    };
    let res = execute(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "extend_lock_amount");

    (deps, anchor_info, owner_info)
}
