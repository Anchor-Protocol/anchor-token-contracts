use crate::error::ContractError;

use crate::contract::{execute, instantiate, query};
use crate::mock_querier::mock_dependencies;

use anchor_token::gauge_controller::{
    AllGaugeAddrResponse, ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeCountResponse,
    GaugeWeightResponse, InstantiateMsg, QueryMsg,
};

use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{from_binary, Deps, DepsMut, Timestamp, Uint128};
use serde::de::DeserializeOwned;

const WEEK: u64 = 7 * 24 * 60 * 60;
const BASE_TIME: u64 = WEEK * 1000 + 10;

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
    if let Err(_) = execute(deps, env, info, msg) {
        panic!("DO NOT ENTER HERE");
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

// test AddGauge, ChangeGaugeWeight, GaugeCount, GaugeWeight, GaugeAddr, AllGaugeAddr
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
        ContractError::GaugeAlreadyExist {},
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
