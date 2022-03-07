use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use crate::utils::{MAX_LOCK_TIME, WEEK};
use anchor_token::voting_escrow::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMarketingInfo, InstantiateMsg,
    LockInfoResponse, QueryMsg, VotingPowerResponse,
};
use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{
    from_binary, to_binary, Decimal, MessageInfo, OwnedDeps, StdError, Timestamp, Uint128,
};
use cw20::{Cw20ReceiveMsg, Logo, LogoInfo, MarketingInfoResponse, TokenInfoResponse};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor".to_string(),
        marketing: Some(InstantiateMarketingInfo {
            project: Some("voted-escrow".to_string()),
            description: Some("voted-escrow".to_string()),
            logo: Some(Logo::Url("votes-escrow-url".to_string())),
            marketing: Some("marketing".to_string()),
        }),
    };

    let info = mock_info("owner", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

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

    let mut receive_msg = Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { time: WEEK }).unwrap(),
    };

    let msg = ExecuteMsg::Receive(receive_msg.clone());

    // only anchor token is authorized to create locks
    let info = mock_info("random", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(ContractError::Unauthorized {}) => {}
        _ => panic!("Must return Unauthorized error"),
    }

    let info = mock_info("anchor", &[]);

    // time provided is below limit
    receive_msg.msg = to_binary(&Cw20HookMsg::CreateLock { time: 2 * 86400 }).unwrap();
    let msg = ExecuteMsg::Receive(receive_msg.clone());
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
    match res {
        Err(ContractError::LockTimeLimitsError {}) => {}
        _ => panic!("Must return LockTimeLimitsError error"),
    }

    // time provided is above limit
    receive_msg.msg = to_binary(&Cw20HookMsg::CreateLock {
        time: MAX_LOCK_TIME + 86400,
    })
    .unwrap();
    let msg = ExecuteMsg::Receive(receive_msg.clone());
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
    match res {
        Err(ContractError::LockTimeLimitsError {}) => {}
        _ => panic!("Must return LockTimeLimitsError error"),
    }

    // creates lock successfully
    receive_msg.msg = to_binary(&Cw20HookMsg::CreateLock { time: 2 * WEEK }).unwrap();
    let msg = ExecuteMsg::Receive(receive_msg.clone());
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "create_lock");

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::LockInfo {
            user: "addr0000".to_string(),
        },
    )
    .unwrap();
    let lock_info: LockInfoResponse = from_binary(&res).unwrap();

    let weeks_in_max_time = Uint128::from(104u64); // 2 years in weeks
    let coeff_in_2_weeks = Uint128::from(3u64); // 1.5 * 2
    let expected_coeff = Decimal::one() + Decimal::from_ratio(coeff_in_2_weeks, weeks_in_max_time);

    assert_eq!(lock_info.amount, Uint128::from(10u128));
    assert_eq!((lock_info.end - lock_info.start) * WEEK, 2 * WEEK);
    assert_eq!(lock_info.coefficient, expected_coeff);

    // cannot create multiple locks for same user
    receive_msg.msg = to_binary(&Cw20HookMsg::CreateLock { time: WEEK }).unwrap();
    let msg = ExecuteMsg::Receive(receive_msg);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
    match res {
        Err(ContractError::LockAlreadyExists {}) => {}
        _ => panic!("Must return LockAlreadyExists error"),
    };

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
    let (mut deps, anchor_info, _) =
        init_lock_factory("addr0000".to_string(), Some(Uint128::from(20u64)), None);

    let mut receive_msg = Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    };

    let msg = ExecuteMsg::Receive(receive_msg.clone());

    // only anchor token is authorized to extend lock amount
    let info = mock_info("random", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());
    match res {
        Err(ContractError::Unauthorized {}) => {}
        _ => panic!("Must return Unauthorized error"),
    };

    // cannot extend lock amount for a user w/o a lock
    receive_msg.sender = "random0000".to_string();
    let msg = ExecuteMsg::Receive(receive_msg.clone());
    let res = execute(deps.as_mut(), mock_env(), anchor_info.clone(), msg);
    match res {
        Err(ContractError::LockDoesntExist {}) => {}
        _ => panic!("Must return LockDoesntExist error"),
    };

    // cannot extend lock amount for an expired lock
    receive_msg.sender = "addr0000".to_string();
    let msg = ExecuteMsg::Receive(receive_msg.clone());
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 3 * WEEK);
    let res = execute(deps.as_mut(), env, anchor_info.clone(), msg.clone());
    match res {
        Err(ContractError::LockExpired {}) => {}
        _ => panic!("Must return LockExpired error"),
    };

    // extends lock amount successfully
    let res = execute(deps.as_mut(), mock_env(), anchor_info.clone(), msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "deposit_for");

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
fn test_deposit_for() {
    let (mut deps, anchor_info, _) =
        init_lock_factory("addr0000".to_string(), Some(Uint128::from(50u128)), None);

    let mut receive_msg = Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::DepositFor {
            user: "addr0000".to_string(),
        })
        .unwrap(),
    };

    let msg = ExecuteMsg::Receive(receive_msg.clone());

    // only anchor token is authorized to deposit for `user`
    let info = mock_info("random", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(ContractError::Unauthorized {}) => {}
        _ => panic!("Must return Unauthorized error"),
    };

    // deposit `user` address must be valid
    receive_msg.msg = to_binary(&Cw20HookMsg::DepositFor {
        user: "UPPER0000".to_string(),
    })
    .unwrap();
    let msg = ExecuteMsg::Receive(receive_msg.clone());
    let res = execute(deps.as_mut(), mock_env(), anchor_info.clone(), msg);
    match res {
        Err(ContractError::Std(StdError::GenericErr { msg })) => {
            assert_eq!(msg, "Address UPPER0000 should be lowercase")
        }
        _ => panic!("Must return address validation error"),
    }

    // deposit for `user` successfully
    receive_msg.sender = anchor_info.sender.to_string();
    receive_msg.amount = Uint128::from(50u128);
    receive_msg.msg = to_binary(&Cw20HookMsg::DepositFor {
        user: "addr0000".to_string(),
    })
    .unwrap();
    let msg = ExecuteMsg::Receive(receive_msg.clone());
    let _res = execute(deps.as_mut(), mock_env(), anchor_info.clone(), msg).unwrap();

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::LockInfo {
            user: "addr0000".to_string(),
        },
    )
    .unwrap();
    let lock_info: LockInfoResponse = from_binary(&res).unwrap();

    assert_eq!(lock_info.amount, Uint128::from(100u64));
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
    let lock_amount = lock_amount.unwrap_or(Uint128::from(10u64));
    let lock_time = lock_time.unwrap_or(WEEK);

    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor".to_string(),
        marketing: None,
    };

    let owner_info = mock_info("owner", &[]);
    let anchor_info = mock_info("anchor", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    // creates lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: user,
        amount: lock_amount,
        msg: to_binary(&Cw20HookMsg::CreateLock { time: lock_time }).unwrap(),
    });
    let res = execute(deps.as_mut(), mock_env(), anchor_info.clone(), msg).unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "create_lock");

    (deps, anchor_info, owner_info)
}
