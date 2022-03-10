use crate::error::ContractError;

use crate::contract::{execute, instantiate, query};
use crate::mock_querier::{mock_dependencies, BASE_TIME};
use crate::utils::{VOTE_DELAY, WEEK};

use anchor_token::gauge_controller::{
    AllGaugeAddrResponse, ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeCountResponse,
    GaugeRelativeWeightResponse, GaugeWeightResponse, InstantiateMsg, QueryMsg,
};

use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{from_binary, Decimal, Deps, DepsMut, Timestamp, Uint128};
use serde::de::DeserializeOwned;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor_token".to_string(),
        anchor_voting_escorw: "anchor_voting_escrow".to_string(),
    };
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!("owner", config.owner.as_str());
    assert_eq!("anchor_token", config.anchor_token.as_str());
    assert_eq!("anchor_voting_escrow", config.anchor_voting_escorw.as_str());
}

fn run_execute_msg_expect_ok(deps: DepsMut, sender: String, msg: ExecuteMsg, time: Option<u64>) {
    let info = mock_info(&sender, &[]);
    let mut env = mock_env();
    env.block.time = if let Some(time) = time {
        Timestamp::from_seconds(time)
    } else {
        Timestamp::from_seconds(BASE_TIME)
    };
    if let Err(err) = execute(deps, env, info, msg) {
        panic!("{}", err);
    }
}

fn run_execute_msg_expect_error(
    deps: DepsMut,
    sender: String,
    msg: ExecuteMsg,
    time: Option<u64>,
) -> ContractError {
    let info = mock_info(&sender, &[]);
    let mut env = mock_env();
    env.block.time = if let Some(time) = time {
        Timestamp::from_seconds(time)
    } else {
        Timestamp::from_seconds(BASE_TIME)
    };
    if let Err(err) = execute(deps, env, info, msg) {
        return err;
    }
    panic!("DO NOT ENTER HERE");
}

fn run_query_msg_expect_ok<T: DeserializeOwned>(deps: Deps, msg: QueryMsg, time: Option<u64>) -> T {
    let mut env = mock_env();
    env.block.time = if let Some(time) = time {
        Timestamp::from_seconds(time)
    } else {
        Timestamp::from_seconds(BASE_TIME)
    };
    from_binary(&query(deps, env, msg).unwrap()).unwrap()
}

fn run_query_msg_expect_error(deps: Deps, msg: QueryMsg, time: Option<u64>) -> ContractError {
    let mut env = mock_env();
    env.block.time = if let Some(time) = time {
        Timestamp::from_seconds(time)
    } else {
        Timestamp::from_seconds(BASE_TIME)
    };
    if let Err(err) = query(deps, env, msg) {
        return err;
    }
    panic!("DO NOT ENTER HERE");
}

#[test]
fn test_vote_for_single_gauge() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escorw: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(23333_u64),
        },
        None,
    );

    assert_eq!(
        ContractError::InvalidVotingRatio {},
        run_execute_msg_expect_error(
            deps.as_mut(),
            "user_1".to_string(),
            ExecuteMsg::VoteForGaugeWeight {
                addr: "gauge_addr_1".to_string(),
                voting_ratio: 10001,
            },
            None,
        )
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            voting_ratio: 10000,
        },
        None,
    );

    assert_eq!(
        ContractError::VoteTooOften {},
        run_execute_msg_expect_error(
            deps.as_mut(),
            "user_1".to_string(),
            ExecuteMsg::VoteForGaugeWeight {
                addr: "gauge_addr_1".to_string(),
                voting_ratio: 10000,
            },
            Some(BASE_TIME + WEEK * (VOTE_DELAY - 1)),
        )
    );

    assert_eq!(
        Uint128::from(23333_u64 + 998244353_u64 - 9982444_u64),
        run_query_msg_expect_ok::<GaugeWeightResponse>(
            deps.as_ref(),
            QueryMsg::GaugeWeight {
                addr: "gauge_addr_1".to_string()
            },
            None,
        )
        .gauge_weight
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::CheckpointAll {},
        Some(BASE_TIME + WEEK * VOTE_DELAY),
    );

    assert_eq!(
        Uint128::from(23333_u64 + 998244353_u64 - 19964888_u64),
        run_query_msg_expect_ok::<GaugeWeightResponse>(
            deps.as_ref(),
            QueryMsg::GaugeWeight {
                addr: "gauge_addr_1".to_string()
            },
            Some(BASE_TIME + WEEK * VOTE_DELAY),
        )
        .gauge_weight
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            voting_ratio: 0,
        },
        Some(BASE_TIME + WEEK * VOTE_DELAY),
    );

    assert_eq!(
        Uint128::from(23332_u64),
        run_query_msg_expect_ok::<GaugeWeightResponse>(
            deps.as_ref(),
            QueryMsg::GaugeWeight {
                addr: "gauge_addr_1".to_string()
            },
            Some(BASE_TIME + WEEK * VOTE_DELAY),
        )
        .gauge_weight
    );
}

/// test AddGauge, ChangeGaugeWeight, GaugeCount, GaugeWeight, TotalWeight,  
/// GaugeRelativeWeight, GaugeAddr, AllGaugeAddr, Config
#[test]
fn test_add_two_gauges_and_change_weight() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escorw: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(100_u64),
        },
        None,
    );

    assert_eq!(
        1,
        run_query_msg_expect_ok::<GaugeCountResponse>(deps.as_ref(), QueryMsg::GaugeCount {}, None)
            .gauge_count
    );

    assert_eq!(
        Uint128::from(100_u64),
        run_query_msg_expect_ok::<GaugeWeightResponse>(
            deps.as_ref(),
            QueryMsg::GaugeWeight {
                addr: "gauge_addr_1".to_string(),
            },
            None,
        )
        .gauge_weight
    );

    assert_eq!(
        "gauge_addr_1".to_string(),
        run_query_msg_expect_ok::<GaugeAddrResponse>(
            deps.as_ref(),
            QueryMsg::GaugeAddr { gauge_id: 0_u64 },
            None,
        )
        .gauge_addr
    );

    assert_eq!(
        ContractError::GaugeNotFound {},
        run_query_msg_expect_error(deps.as_ref(), QueryMsg::GaugeAddr { gauge_id: 1_u64 }, None)
    );

    assert_eq!(
        ContractError::GaugeAlreadyExists {},
        run_execute_msg_expect_error(
            deps.as_mut(),
            "owner".to_string(),
            ExecuteMsg::AddGauge {
                addr: "gauge_addr_1".to_string(),
                weight: Uint128::from(100_u64),
            },
            None,
        )
    );

    assert_eq!(
        ContractError::Unauthorized {},
        run_execute_msg_expect_error(
            deps.as_mut(),
            "addr0000".to_string(),
            ExecuteMsg::AddGauge {
                addr: "gauge_addr_2".to_string(),
                weight: Uint128::from(100_u64),
            },
            None,
        )
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(100_u64),
        },
        None,
    );

    assert_eq!(
        2,
        run_query_msg_expect_ok::<GaugeCountResponse>(deps.as_ref(), QueryMsg::GaugeCount {}, None)
            .gauge_count
    );

    assert_eq!(
        vec!["gauge_addr_1".to_string(), "gauge_addr_2".to_string()],
        run_query_msg_expect_ok::<AllGaugeAddrResponse>(
            deps.as_ref(),
            QueryMsg::AllGaugeAddr {},
            None
        )
        .all_gauge_addr
    );

    assert_eq!(
        ContractError::Unauthorized {},
        run_execute_msg_expect_error(
            deps.as_mut(),
            "addr0000".to_string(),
            ExecuteMsg::ChangeGaugeWeight {
                addr: "gauge_addr_1".to_string(),
                weight: Uint128::from(200_u64),
            },
            None,
        )
    );

    assert_eq!(
        ContractError::GaugeNotFound {},
        run_execute_msg_expect_error(
            deps.as_mut(),
            "owner".to_string(),
            ExecuteMsg::ChangeGaugeWeight {
                addr: "gauge_addr_3".to_string(),
                weight: Uint128::from(200_u64),
            },
            None,
        )
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(200_u64),
        },
        None,
    );

    assert_eq!(
        Uint128::from(200_u64),
        run_query_msg_expect_ok::<GaugeWeightResponse>(
            deps.as_ref(),
            QueryMsg::GaugeWeight {
                addr: "gauge_addr_1".to_string(),
            },
            None,
        )
        .gauge_weight
    );

    assert_eq!(
        Decimal::from_ratio(Uint128::from(2_u64), Uint128::from(3_u64)),
        run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
            deps.as_ref(),
            QueryMsg::GaugeRelativeWeight {
                addr: "gauge_addr_1".to_string(),
            },
            None,
        )
        .gauge_relative_weight
    );

    assert_eq!(
        ContractError::TimestampError {},
        run_execute_msg_expect_error(
            deps.as_mut(),
            "owner".to_string(),
            ExecuteMsg::ChangeGaugeWeight {
                addr: "gauge_addr_2".to_string(),
                weight: Uint128::from(200_u64),
            },
            Some(BASE_TIME - WEEK * 10),
        )
    );
}
