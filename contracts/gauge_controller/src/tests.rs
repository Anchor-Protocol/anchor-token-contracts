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

fn run_execute_msg_expect_ok(deps: DepsMut, sender: String, msg: ExecuteMsg, time: u64) {
    let info = mock_info(&sender, &[]);
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    if let Err(err) = execute(deps, env, info, msg) {
        panic!("{}", err);
    }
}

fn run_execute_msg_expect_error(
    expect_err: ContractError,
    deps: DepsMut,
    sender: String,
    msg: ExecuteMsg,
    time: u64,
) {
    let info = mock_info(&sender, &[]);
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    match execute(deps, env, info, msg) {
        Err(err) => assert_eq!(expect_err, err),
        _ => panic!("DO NOT ENTER HERE"),
    };
}

fn run_query_msg_expect_ok<T: DeserializeOwned + std::cmp::PartialEq + std::fmt::Debug>(
    expect_response: T,
    deps: Deps,
    msg: QueryMsg,
    time: u64,
) {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    assert_eq!(
        expect_response,
        from_binary(&query(deps, env, msg).unwrap()).unwrap()
    );
}

fn run_query_msg_expect_error(expect_err: ContractError, deps: Deps, msg: QueryMsg, time: u64) {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    match query(deps, env, msg) {
        Err(err) => assert_eq!(expect_err, err),
        _ => panic!("DO NOT ENTER HERE"),
    };
}

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

    let time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(100_u64),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeCountResponse>(
        GaugeCountResponse { gauge_count: 1 },
        deps.as_ref(),
        QueryMsg::GaugeCount {},
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(100_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeAddrResponse>(
        GaugeAddrResponse {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        deps.as_ref(),
        QueryMsg::GaugeAddr { gauge_id: 0_u64 },
        time,
    );

    run_query_msg_expect_error(
        ContractError::GaugeNotFound {},
        deps.as_ref(),
        QueryMsg::GaugeAddr { gauge_id: 1_u64 },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::GaugeAlreadyExists {},
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(100_u64),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::Unauthorized {},
        deps.as_mut(),
        "addr0000".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(100_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(100_u64),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeCountResponse>(
        GaugeCountResponse { gauge_count: 2 },
        deps.as_ref(),
        QueryMsg::GaugeCount {},
        time,
    );

    run_query_msg_expect_ok::<AllGaugeAddrResponse>(
        AllGaugeAddrResponse {
            all_gauge_addr: vec!["gauge_addr_1".to_string(), "gauge_addr_2".to_string()],
        },
        deps.as_ref(),
        QueryMsg::AllGaugeAddr {},
        time,
    );

    run_execute_msg_expect_error(
        ContractError::Unauthorized {},
        deps.as_mut(),
        "addr0000".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(200_u64),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::GaugeNotFound {},
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            addr: "gauge_addr_3".to_string(),
            weight: Uint128::from(200_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(200_u64),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(200_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
        GaugeRelativeWeightResponse {
            gauge_relative_weight: Decimal::from_ratio(Uint128::from(2_u64), Uint128::from(3_u64)),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::TimestampError {},
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(200_u64),
        },
        time - WEEK,
    );
}

#[test]
fn test_vote_for_single_gauge_by_single_user() {
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

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(23333_u64),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::InvalidVotingRatio {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            ratio: 10001,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            ratio: 10000,
        },
        time,
    );

    time += WEEK * (VOTE_DELAY - 1);

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::CheckpointGauge {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::VoteTooOften {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            ratio: 10000,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(988285242_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::CheckpointGauge {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(978302798_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            ratio: 0,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(23332_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 100 * WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::CheckpointGauge {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(23332_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::LockExpiresTooSoon {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            ratio: 10000,
        },
        time,
    );
}

#[test]
fn test_vote_for_single_gauge_by_multiple_users() {
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

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(0_u64),
        },
        time,
    );

    run_query_msg_expect_error(
        ContractError::TotalWeightIsZero {},
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(23333_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            ratio: 768,
        },
        time,
    );

    time += WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_2".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            ratio: 8453,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(908414277_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 23 * WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(908414277_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_3".to_string(),
        ExecuteMsg::CheckpointGauge {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(596207044_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 42 * WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_3".to_string(),
        ExecuteMsg::CheckpointGauge {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(26089489_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 8 * WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_3".to_string(),
        ExecuteMsg::CheckpointGauge {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(19956276_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );
}

#[test]
fn test_vote_for_multiple_gauges_by_single_user() {
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

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(23333_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(66666_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_1".to_string(),
            ratio: 4357,
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::InsufficientVotingRatio {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_2".to_string(),
            ratio: 5644,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            addr: "gauge_addr_2".to_string(),
            ratio: 5643,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
        GaugeRelativeWeightResponse {
            gauge_relative_weight: Decimal::from_ratio(
                Uint128::from(434958398_u64),
                Uint128::from(563375954_u64 + 434958398_u64),
            ),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 17 * WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_3".to_string(),
        ExecuteMsg::CheckpointAll {},
        time,
    );

    run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
        GaugeRelativeWeightResponse {
            gauge_relative_weight: Decimal::from_ratio(
                Uint128::from(361019437_u64),
                Uint128::from(467613375_u64 + 361019437_u64),
            ),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 100 * WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_3".to_string(),
        ExecuteMsg::CheckpointAll {},
        time,
    );

    run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
        GaugeRelativeWeightResponse {
            gauge_relative_weight: Decimal::from_ratio(
                Uint128::from(23333_u64),
                Uint128::from(66666_u64 + 23333_u64),
            ),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            addr: "gauge_addr_1".to_string(),
        },
        time,
    );
}
