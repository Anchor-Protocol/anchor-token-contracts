use crate::contract::{execute, instantiate, migrate, query, reply};
use crate::error::ContractError;
use crate::migration::{read_legacy_config, LegacyConfig};
use crate::mock_querier::mock_dependencies;
use crate::state::{
    bank_read, bank_store, config_read, config_store, poll_store, poll_voter_read,
    poll_voter_store, state_read, Config, Poll, State, TokenManager,
};
use anchor_token::common::OrderBy;
use anchor_token::gov::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PollExecuteMsg,
    PollResponse, PollStatus, PollsResponse, QueryMsg, StakerResponse, VoteOption, VoterInfo,
    VotersResponse, VotersResponseItem,
};
use astroport::querier::query_token_balance;
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, coins, from_binary, to_binary, Addr, Api, CanonicalAddr, ContractResult, CosmosMsg,
    Decimal, Deps, DepsMut, Env, Reply, Response, StdError, SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

const VOTING_TOKEN: &str = "voting_token";
const VOTING_ESCROW: &str = "voting_escrow";
const TEST_CREATOR: &str = "creator";
const TEST_VOTER: &str = "voter1";
const TEST_VOTER_2: &str = "voter2";
const TEST_VOTER_3: &str = "voter3";
const DEFAULT_QUORUM: u64 = 30u64;
const DEFAULT_THRESHOLD: u64 = 50u64;
const DEFAULT_VOTING_PERIOD: u64 = 20000u64;
const DEFAULT_FIX_PERIOD: u64 = 10u64;
const DEFAULT_TIMELOCK_PERIOD: u64 = 10000u64;
const DEFAULT_PROPOSAL_DEPOSIT: u128 = 10000000000u128;

fn mock_instantiate(deps: DepsMut) {
    let msg = InstantiateMsg {
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        timelock_period: DEFAULT_TIMELOCK_PERIOD,
        proposal_deposit: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
        snapshot_period: DEFAULT_FIX_PERIOD,
    };

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps, mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");
}

fn mock_register_contracts(deps: DepsMut) {
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::RegisterContracts {
        anchor_token: VOTING_TOKEN.to_string(),
        anchor_voting_escrow: VOTING_ESCROW.to_string(),
    };
    let _res = execute(deps, mock_env(), info, msg)
        .expect("contract successfully handles RegisterContracts");
}

fn mock_env_height(height: u64, time: u64) -> Env {
    let mut env = mock_env();
    env.block.height = height;
    env.block.time = Timestamp::from_seconds(time);
    env
}

fn instantiate_msg() -> InstantiateMsg {
    InstantiateMsg {
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        timelock_period: DEFAULT_TIMELOCK_PERIOD,
        proposal_deposit: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
        snapshot_period: DEFAULT_FIX_PERIOD,
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = instantiate_msg();
    let info = mock_info(TEST_CREATOR, &coins(2, VOTING_TOKEN));
    let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let config: Config = config_read(deps.as_ref().storage).load().unwrap();
    assert_eq!(
        config,
        Config {
            anchor_token: CanonicalAddr::from(vec![]),
            anchor_voting_escrow: CanonicalAddr::from(vec![]),
            owner: deps.api.addr_canonicalize(TEST_CREATOR).unwrap(),
            quorum: Decimal::percent(DEFAULT_QUORUM),
            threshold: Decimal::percent(DEFAULT_THRESHOLD),
            voting_period: DEFAULT_VOTING_PERIOD,
            timelock_period: DEFAULT_TIMELOCK_PERIOD,
            expiration_period: 0u64, // Deprecated
            proposal_deposit: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            snapshot_period: DEFAULT_FIX_PERIOD
        }
    );

    let msg = ExecuteMsg::RegisterContracts {
        anchor_token: VOTING_TOKEN.to_string(),
        anchor_voting_escrow: VOTING_ESCROW.to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let config: Config = config_read(deps.as_ref().storage).load().unwrap();
    assert_eq!(
        config.anchor_token,
        deps.api.addr_canonicalize(VOTING_TOKEN).unwrap()
    );

    let state: State = state_read(deps.as_ref().storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::zero(),
            total_deposit: Uint128::zero(),
        }
    );
}

#[test]
fn poll_not_found() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 });

    match res {
        Err(ContractError::PollNotFound {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
        _ => panic!("Must return error"),
    }
}

#[test]
fn fails_init_invalid_quorum() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("voter", &coins(11, VOTING_TOKEN));
    let msg = InstantiateMsg {
        quorum: Decimal::percent(101),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        timelock_period: DEFAULT_TIMELOCK_PERIOD,
        proposal_deposit: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
        snapshot_period: DEFAULT_FIX_PERIOD,
    };

    let res = instantiate(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "quorum must be 0 to 1")
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_init_invalid_threshold() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("voter", &coins(11, VOTING_TOKEN));
    let msg = InstantiateMsg {
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(101),
        voting_period: DEFAULT_VOTING_PERIOD,
        timelock_period: DEFAULT_TIMELOCK_PERIOD,
        proposal_deposit: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
        snapshot_period: DEFAULT_FIX_PERIOD,
    };

    let res = instantiate(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "threshold must be 0 to 1")
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_contract_already_registered() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("voter", &coins(11, VOTING_TOKEN));
    let msg = InstantiateMsg {
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        timelock_period: DEFAULT_TIMELOCK_PERIOD,
        proposal_deposit: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
        snapshot_period: DEFAULT_FIX_PERIOD,
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::RegisterContracts {
        anchor_token: VOTING_TOKEN.to_string(),
        anchor_voting_escrow: VOTING_ESCROW.to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Unauthorized { .. }) => {}
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_create_poll_invalid_title() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let msg = create_poll_msg("a".to_string(), "test".to_string(), None, None);
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "Title too short")
        }
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
            "0123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234".to_string(),
            "test".to_string(),
            None,
            None,
        );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "Title too long")
        }
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_description() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "a".to_string(), None, None);
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "Description too short")
        }
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
            "test".to_string(),
            "012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678900123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789001234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012341234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123456".to_string(),
            None,
            None,
        );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "Description too long")
        }
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_link() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        Some("http://hih".to_string()),
        None,
    );
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "Link too short")
        }
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        Some("0123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234".to_string()),
        None,
    );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "Link too long")
        }
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_deposit() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_CREATOR.to_string(),
        amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT - 1),
        msg: to_binary(&Cw20HookMsg::CreatePoll {
            title: "TESTTEST".to_string(),
            description: "TESTTEST".to_string(),
            link: None,
            execute_msgs: None,
        })
        .unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::InsufficientProposalDeposit(DEFAULT_PROPOSAL_DEPOSIT)) => (),
        Err(_) => panic!("Unknown error"),
    }
}

fn create_poll_msg(
    title: String,
    description: String,
    link: Option<String>,
    execute_msg: Option<Vec<PollExecuteMsg>>,
) -> ExecuteMsg {
    ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_CREATOR.to_string(),
        amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
        msg: to_binary(&Cw20HookMsg::CreatePoll {
            title,
            description,
            link,
            execute_msgs: execute_msg,
        })
        .unwrap(),
    })
}

#[test]
fn happy_days_create_poll() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_create_poll_result(
        1,
        env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );
}

#[test]
fn query_polls() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    let exec_msg_bz2 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(12),
    })
    .unwrap();

    let exec_msg_bz3 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(1),
    })
    .unwrap();

    let execute_msgs: Vec<PollExecuteMsg> = vec![
        PollExecuteMsg {
            order: 1u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        },
        PollExecuteMsg {
            order: 3u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz3,
        },
        PollExecuteMsg {
            order: 2u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz2,
        },
    ];

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        Some("http://google.com".to_string()),
        Some(execute_msgs.clone()),
    );

    let _execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let msg = create_poll_msg("test2".to_string(), "test2".to_string(), None, None);
    let _execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![
            PollResponse {
                id: 1u64,
                creator: TEST_CREATOR.to_string(),
                status: PollStatus::InProgress,
                end_height: 20000u64,
                title: "test".to_string(),
                description: "test".to_string(),
                link: Some("http://google.com".to_string()),
                deposit_amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
                execute_data: Some(execute_msgs.clone()),
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                staked_amount: None,
                total_balance_at_end_poll: None,
            },
            PollResponse {
                id: 2u64,
                creator: TEST_CREATOR.to_string(),
                status: PollStatus::InProgress,
                end_height: 20000u64,
                title: "test2".to_string(),
                description: "test2".to_string(),
                link: None,
                deposit_amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
                execute_data: None,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                staked_amount: None,
                total_balance_at_end_poll: None,
            },
        ]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: Some(1u64),
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 2u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_height: 20000u64,
            title: "test2".to_string(),
            description: "test2".to_string(),
            link: None,
            deposit_amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: None,
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            staked_amount: None,
            total_balance_at_end_poll: None,
        },]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: Some(2u64),
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 1u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_height: 20000u64,
            title: "test".to_string(),
            description: "test".to_string(),
            link: Some("http://google.com".to_string()),
            deposit_amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: Some(execute_msgs),
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            staked_amount: None,
            total_balance_at_end_poll: None,
        }]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: Some(1u64),
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 2u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_height: 20000u64,
            title: "test2".to_string(),
            description: "test2".to_string(),
            link: None,
            deposit_amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: None,
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            staked_amount: None,
            total_balance_at_end_poll: None,
        },]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls, vec![]);
}

#[test]
fn create_poll_no_quorum() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let info = mock_info(VOTING_TOKEN, &[]);
    let env = mock_env_height(0, 10000);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );
}

#[test]
fn fails_end_poll_before_end_height() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(DEFAULT_VOTING_PERIOD, value.end_height);

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_CREATOR, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg);

    match execute_res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::PollVotingPeriod {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_end_poll() {
    const POLL_START_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut creator_env = mock_env_height(POLL_START_HEIGHT, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    let exec_msg_bz2 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(12),
    })
    .unwrap();

    let exec_msg_bz3 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(1),
    })
    .unwrap();

    //add three messages with different order
    let execute_msgs: Vec<PollExecuteMsg> = vec![
        PollExecuteMsg {
            order: 3u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz3.clone(),
        },
        PollExecuteMsg {
            order: 2u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz2.clone(),
        },
        PollExecuteMsg {
            order: 1u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz.clone(),
        },
    ];

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(execute_msgs),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(POLL_START_HEIGHT, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    // not in passed status
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap_err();
    match execute_res {
        ContractError::PollNotPassed {} => (),
        _ => panic!("DO NOT ENTER HERE"),
    }

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += DEFAULT_VOTING_PERIOD;

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // End poll will withdraw deposit balance
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(stake_amount as u128),
            )],
        ),
    ]);

    // timelock_period has not expired
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap_err();
    match execute_res {
        ContractError::TimelockNotExpired {} => (),
        _ => panic!("DO NOT ENTER HERE"),
    }

    creator_env.block.height += DEFAULT_TIMELOCK_PERIOD;
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env.clone(), creator_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: creator_env.contract.address.to_string(),
                msg: to_binary(&ExecuteMsg::ExecutePollMsgs { poll_id: 1 }).unwrap(),
                funds: vec![],
            }),
            1
        )]
    );

    let msg = ExecuteMsg::ExecutePollMsgs { poll_id: 1 };
    let contract_info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let execute_res = execute(deps.as_mut(), creator_env, contract_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz,
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz2,
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz3,
                funds: vec![],
            }))
        ]
    );
    assert_eq!(
        execute_res.attributes,
        vec![attr("action", "execute_poll"), attr("poll_id", "1"),]
    );

    // Query executed polls
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Executed),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 1);

    // voter info must be deleted
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(response.voters.len(), 0);

    // staker locked token must be disappeared
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::from(stake_amount),
            share: Uint128::from(stake_amount),
            locked_balance: vec![]
        }
    );

    // But the data is still in the store
    let voter_addr_raw = deps.api.addr_canonicalize(TEST_VOTER).unwrap();
    let voter = poll_voter_read(&deps.storage, 1u64)
        .load(voter_addr_raw.as_slice())
        .unwrap();
    assert_eq!(
        voter,
        VoterInfo {
            vote: VoteOption::Yes,
            balance: Uint128::from(stake_amount),
        }
    );

    let token_manager = bank_read(&deps.storage)
        .load(voter_addr_raw.as_slice())
        .unwrap();
    assert_eq!(
        token_manager.locked_balance,
        vec![(
            1u64,
            VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::from(stake_amount),
            }
        )]
    );
}

#[test]
fn fail_poll() {
    const POLL_START_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut creator_env = mock_env_height(POLL_START_HEIGHT, 10000);
    let creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();
    let execute_msgs: Vec<PollExecuteMsg> = vec![PollExecuteMsg {
        order: 1u64,
        contract: VOTING_TOKEN.to_string(),
        msg: exec_msg_bz.clone(),
    }];
    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(execute_msgs),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(POLL_START_HEIGHT, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_env.block.height += DEFAULT_VOTING_PERIOD;

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // Execute Poll should send submsg ExecutePollMsgs
    creator_env.block.height += DEFAULT_TIMELOCK_PERIOD;
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env.clone(), creator_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: creator_env.contract.address.to_string(),
                msg: to_binary(&ExecuteMsg::ExecutePollMsgs { poll_id: 1 }).unwrap(),
                funds: vec![],
            }),
            1
        )]
    );

    // ExecutePollMsgs should send poll messages
    let msg = ExecuteMsg::ExecutePollMsgs { poll_id: 1 };
    let contract_info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let execute_res = execute(deps.as_mut(), creator_env, contract_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
            funds: vec![],
        }))]
    );

    // invalid reply id
    let reply_msg = Reply {
        id: 2,
        result: ContractResult::Err("Error".to_string()),
    };
    let res = reply(deps.as_mut(), mock_env(), reply_msg);
    assert_eq!(res, Err(ContractError::InvalidReplyId {}));

    // correct reply id
    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Err("Error".to_string()),
    };
    let res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![attr("action", "fail_poll"), attr("poll_id", "1")]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let poll_res: PollResponse = from_binary(&res).unwrap();
    assert_eq!(poll_res.status, PollStatus::Failed);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Failed),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let polls_res: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(polls_res.polls[0], poll_res);
}

#[test]
fn end_poll_zero_quorum() {
    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut creator_env = mock_env_height(1000, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);

    let execute_msgs: Vec<PollExecuteMsg> = vec![PollExecuteMsg {
        order: 1u64,
        contract: VOTING_TOKEN.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Burn {
            amount: Uint128::new(123),
        })
        .unwrap(),
    }];

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(execute_msgs),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    assert_create_poll_result(
        1,
        creator_env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );
    let stake_amount = 100;
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(100u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += DEFAULT_VOTING_PERIOD;

    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );

    assert_eq!(execute_res.messages.len(), 0usize);

    // Query rejected polls
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Rejected),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 1);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);
}

#[test]
fn end_poll_quorum_rejected() {
    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let mut creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_height", "32345"),
        ]
    );

    let stake_amount = 100;
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(100u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", "1"),
            attr("amount", "10"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += DEFAULT_VOTING_PERIOD;

    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn end_poll_quorum_rejected_nothing_staked() {
    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let mut creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_height", "32345"),
        ]
    );

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += DEFAULT_VOTING_PERIOD;

    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn end_poll_nay_rejected() {
    let voter1_stake = 100;
    let voter2_stake = 1000;
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_height", "32345"),
        ]
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(voter1_stake))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((voter1_stake + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(voter1_stake as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        voter1_stake,
        DEFAULT_PROPOSAL_DEPOSIT,
        voter1_stake,
        1,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[
                (&TEST_VOTER.to_string(), &Uint128::from(voter1_stake)),
                (&TEST_VOTER_2.to_string(), &Uint128::from(voter2_stake)),
            ],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((voter1_stake + voter2_stake + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(voter2_stake as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        voter1_stake + voter2_stake,
        DEFAULT_PROPOSAL_DEPOSIT,
        voter2_stake,
        1,
        execute_res,
        deps.as_ref(),
    );

    let info = mock_info(TEST_VOTER_2, &[]);
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::No,
        amount: Uint128::from(voter2_stake),
    };
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, voter2_stake, 1, VoteOption::No, execute_res);

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += DEFAULT_VOTING_PERIOD;
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Threshold not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn fails_cast_vote_not_enough_staked() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(10_u64))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(10u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        10,
        DEFAULT_PROPOSAL_DEPOSIT,
        10,
        1,
        execute_res,
        deps.as_ref(),
    );

    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(11u128),
    };

    let res = execute(deps.as_mut(), env, info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::InsufficientStaked {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_cast_vote() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(11u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(11u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        11,
        DEFAULT_PROPOSAL_DEPOSIT,
        11,
        1,
        execute_res,
        deps.as_ref(),
    );

    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let amount = 10u128;
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    // balance be double
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(22u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(22u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    // Query staker
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::from(22u128),
            share: Uint128::from(11u128),
            locked_balance: vec![(
                1u64,
                VoterInfo {
                    vote: VoteOption::Yes,
                    balance: Uint128::from(amount),
                }
            )]
        }
    );

    // Query voters
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.voters,
        vec![VotersResponseItem {
            voter: TEST_VOTER.to_string(),
            vote: VoteOption::Yes,
            balance: Uint128::from(amount),
        }]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: Some(TEST_VOTER.to_string()),
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(response.voters.len(), 0);
}

#[test]
fn happy_days_withdraw_voting_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(11u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(11u128))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, 0, 11, 0, execute_res, deps.as_ref());

    let state: State = state_read(deps.as_ref().storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::from(11u128),
            total_deposit: Uint128::zero(),
        }
    );

    // double the balance, only half will be withdrawn
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(22u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(22u128))],
        ),
    ]);

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = execute_res.messages.get(0).expect("no message");

    assert_eq!(
        msg,
        &SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(11u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    let state: State = state_read(deps.as_ref().storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::from(6u128),
            total_deposit: Uint128::zero(),
        }
    );
}

#[test]
fn happy_days_withdraw_voting_tokens_all() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(11u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(11u128))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, 0, 11, 0, execute_res, deps.as_ref());

    let state: State = state_read(deps.as_ref().storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::from(11u128),
            total_deposit: Uint128::zero(),
        }
    );

    // double the balance, all balance withdrawn
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(22u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(22u128))],
        ),
    ]);

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens { amount: None };

    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = execute_res.messages.get(0).expect("no message");

    assert_eq!(
        msg,
        &SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(22u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    let state: State = state_read(deps.as_ref().storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::zero(),
            total_deposit: Uint128::zero(),
        }
    );
}

#[test]
fn withdraw_voting_tokens_remove_not_in_progress_poll_voter_info() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(11u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(11u128))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, 0, 11, 0, execute_res, deps.as_ref());

    // make fake polls; one in progress & one in passed
    poll_store(&mut deps.storage)
        .save(
            &1u64.to_be_bytes(),
            &Poll {
                id: 1u64,
                creator: CanonicalAddr::from(vec![]),
                status: PollStatus::InProgress,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                end_height: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                total_balance_at_end_poll: None,
                staked_amount: None,
            },
        )
        .unwrap();

    poll_store(&mut deps.storage)
        .save(
            &2u64.to_be_bytes(),
            &Poll {
                id: 1u64,
                creator: CanonicalAddr::from(vec![]),
                status: PollStatus::Passed,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                end_height: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                total_balance_at_end_poll: None,
                staked_amount: None,
            },
        )
        .unwrap();

    let voter_addr_raw = deps.api.addr_canonicalize(TEST_VOTER).unwrap();
    poll_voter_store(&mut deps.storage, 1u64)
        .save(
            voter_addr_raw.as_slice(),
            &VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::from(5u128),
            },
        )
        .unwrap();
    poll_voter_store(&mut deps.storage, 2u64)
        .save(
            voter_addr_raw.as_slice(),
            &VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::from(5u128),
            },
        )
        .unwrap();
    bank_store(&mut deps.storage)
        .save(
            voter_addr_raw.as_slice(),
            &TokenManager {
                share: Uint128::from(11u128),
                locked_balance: vec![
                    (
                        1u64,
                        VoterInfo {
                            vote: VoteOption::Yes,
                            balance: Uint128::from(5u128),
                        },
                    ),
                    (
                        2u64,
                        VoterInfo {
                            vote: VoteOption::Yes,
                            balance: Uint128::from(5u128),
                        },
                    ),
                ],
            },
        )
        .unwrap();

    // withdraw voting token must remove not in-progress votes infos from the store
    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(5u128)),
    };

    let _ = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let voter = poll_voter_read(&deps.storage, 1u64)
        .load(voter_addr_raw.as_slice())
        .unwrap();
    assert_eq!(
        voter,
        VoterInfo {
            vote: VoteOption::Yes,
            balance: Uint128::from(5u128),
        }
    );
    assert!(poll_voter_read(&deps.storage, 2u64)
        .load(voter_addr_raw.as_slice())
        .is_err());

    let token_manager = bank_read(&deps.storage)
        .load(voter_addr_raw.as_slice())
        .unwrap();
    assert_eq!(
        token_manager.locked_balance,
        vec![(
            1u64,
            VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::from(5u128),
            }
        )]
    );
}

#[test]
fn fails_withdraw_voting_tokens_no_stake() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::NothingStaked {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_withdraw_too_many_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(10u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(10u128))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(10, 0, 10, 0, execute_res, deps.as_ref());

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::InvalidWithdrawAmount {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_cast_vote_twice() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_create_poll_result(
        1,
        env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(11u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(11u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        11,
        DEFAULT_PROPOSAL_DEPOSIT,
        11,
        1,
        execute_res,
        deps.as_ref(),
    );

    let amount = 1u128;
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };
    let res = execute(deps.as_mut(), env, info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::AlreadyVoted {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_cast_vote_without_poll() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let msg = ExecuteMsg::CastVote {
        poll_id: 0,
        vote: VoteOption::Yes,
        amount: Uint128::from(1u128),
    };
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::PollNotFound {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_stake_voting_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(11u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(11u128))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, 0, 11, 0, execute_res, deps.as_ref());
}

#[test]
fn fails_insufficient_funds() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    // insufficient token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(0u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::InsufficientFunds {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_staking_wrong_token() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(11u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(11u128))],
        ),
    ]);

    // wrong token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(&(VOTING_TOKEN.to_string() + "2"), &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(ContractError::Unauthorized {}) => (),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn share_calculation() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    // create 100 share
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(100u128))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg);

    // add more balance(100) to make share:balance = 1:2
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(200u128 + 100u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "extend_lock_amount"),
            attr("sender", TEST_VOTER),
            attr("share", "50"),
            attr("amount", "100"),
        ]
    );

    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(100u128)),
    };
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw"),
            attr("recipient", TEST_VOTER),
            attr("amount", "100"),
        ]
    );

    // 100 tokens withdrawn
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(200u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(200u128))],
        ),
    ]);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let stake_info: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(stake_info.share, Uint128::new(100));
    assert_eq!(stake_info.balance, Uint128::new(200));
    assert_eq!(stake_info.locked_balance, vec![]);
}

// helper to confirm the expected create_poll response
fn assert_create_poll_result(
    poll_id: u64,
    end_height: u64,
    creator: &str,
    execute_res: Response,
    deps: Deps,
) {
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", creator),
            attr("poll_id", poll_id.to_string()),
            attr("end_height", end_height.to_string()),
        ]
    );

    //confirm poll count
    let state: State = state_read(deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 1,
            total_share: Uint128::zero(),
            total_deposit: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
        }
    );
}

fn assert_stake_tokens_result(
    total_share: u128,
    total_deposit: u128,
    new_share: u128,
    poll_count: u64,
    execute_res: Response,
    deps: Deps,
) {
    assert_eq!(
        execute_res.attributes.get(2).expect("no log"),
        &attr("share", new_share.to_string())
    );

    let state: State = state_read(deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count,
            total_share: Uint128::from(total_share),
            total_deposit: Uint128::from(total_deposit),
        }
    );
}

fn assert_cast_vote_success(
    voter: &str,
    amount: u128,
    poll_id: u64,
    vote_option: VoteOption,
    execute_res: Response,
) {
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", poll_id.to_string()),
            attr("amount", amount.to_string()),
            attr("voter", voter),
            attr("vote_option", vote_option.to_string()),
        ]
    );
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    // update owner
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("addr0001".to_string()),
        quorum: None,
        threshold: None,
        voting_period: None,
        timelock_period: None,
        proposal_deposit: None,
        snapshot_period: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!(Decimal::percent(DEFAULT_QUORUM), config.quorum);
    assert_eq!(Decimal::percent(DEFAULT_THRESHOLD), config.threshold);
    assert_eq!(DEFAULT_VOTING_PERIOD, config.voting_period);
    assert_eq!(DEFAULT_TIMELOCK_PERIOD, config.timelock_period);
    assert_eq!(DEFAULT_PROPOSAL_DEPOSIT, config.proposal_deposit.u128());

    // update left items
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        quorum: Some(Decimal::percent(20)),
        threshold: Some(Decimal::percent(75)),
        voting_period: Some(20000u64),
        timelock_period: Some(20000u64),
        proposal_deposit: Some(Uint128::from(123u128)),
        snapshot_period: Some(11),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!(Decimal::percent(20), config.quorum);
    assert_eq!(Decimal::percent(75), config.threshold);
    assert_eq!(20000u64, config.voting_period);
    assert_eq!(20000u64, config.timelock_period);
    assert_eq!(123u128, config.proposal_deposit.u128());
    assert_eq!(11u64, config.snapshot_period);

    // Unauthorzied err
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        quorum: None,
        threshold: None,
        voting_period: None,
        timelock_period: None,
        proposal_deposit: None,
        snapshot_period: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(ContractError::Unauthorized {}) => (),
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn add_several_execute_msgs() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let info = mock_info(VOTING_TOKEN, &[]);
    let env = mock_env_height(0, 10000);

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    let exec_msg_bz2 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(12),
    })
    .unwrap();

    let exec_msg_bz3 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(1),
    })
    .unwrap();

    // push two execute msgs to the list
    let execute_msgs: Vec<PollExecuteMsg> = vec![
        PollExecuteMsg {
            order: 1u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        },
        PollExecuteMsg {
            order: 3u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz3,
        },
        PollExecuteMsg {
            order: 2u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz2,
        },
    ];

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(execute_msgs.clone()),
    );

    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_create_poll_result(
        1,
        env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();

    let response_execute_data = value.execute_data.unwrap();
    assert_eq!(response_execute_data.len(), 3);
    assert_eq!(response_execute_data, execute_msgs);
}

#[test]
fn execute_poll_with_order() {
    const POLL_START_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut creator_env = mock_env_height(POLL_START_HEIGHT, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(10),
    })
    .unwrap();

    let exec_msg_bz2 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(20),
    })
    .unwrap();

    let exec_msg_bz3 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(30),
    })
    .unwrap();
    let exec_msg_bz4 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(40),
    })
    .unwrap();
    let exec_msg_bz5 = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(50),
    })
    .unwrap();

    //add three messages with different order
    let execute_msgs: Vec<PollExecuteMsg> = vec![
        PollExecuteMsg {
            order: 3u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz3.clone(),
        },
        PollExecuteMsg {
            order: 4u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz4.clone(),
        },
        PollExecuteMsg {
            order: 2u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz2.clone(),
        },
        PollExecuteMsg {
            order: 5u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz5.clone(),
        },
        PollExecuteMsg {
            order: 1u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz.clone(),
        },
    ];

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(execute_msgs),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(POLL_START_HEIGHT, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += DEFAULT_VOTING_PERIOD;

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // End poll will withdraw deposit balance
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(stake_amount as u128),
            )],
        ),
    ]);

    creator_env.block.height += DEFAULT_TIMELOCK_PERIOD;
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env.clone(), creator_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: creator_env.contract.address.to_string(),
                msg: to_binary(&ExecuteMsg::ExecutePollMsgs { poll_id: 1 }).unwrap(),
                funds: vec![],
            }),
            1
        )]
    );

    let msg = ExecuteMsg::ExecutePollMsgs { poll_id: 1 };
    let contract_info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let execute_res = execute(deps.as_mut(), creator_env, contract_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz,
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz2,
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz3,
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz4,
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz5,
                funds: vec![],
            })),
        ]
    );
    assert_eq!(
        execute_res.attributes,
        vec![attr("action", "execute_poll"), attr("poll_id", "1"),]
    );
}

#[test]
fn poll_with_empty_execute_data_marked_as_executed() {
    const POLL_START_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut creator_env = mock_env_height(POLL_START_HEIGHT, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, Some(vec![]));

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(POLL_START_HEIGHT, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += DEFAULT_VOTING_PERIOD;

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // End poll will withdraw deposit balance
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(stake_amount as u128),
            )],
        ),
    ]);

    creator_env.block.height += DEFAULT_TIMELOCK_PERIOD;
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env.clone(), creator_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: creator_env.contract.address.to_string(),
                msg: to_binary(&ExecuteMsg::ExecutePollMsgs { poll_id: 1 }).unwrap(),
                funds: vec![],
            }),
            1
        )]
    );

    // Executes since empty polls are allowed
    let msg = ExecuteMsg::ExecutePollMsgs { poll_id: 1 };
    let contract_info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let execute_res = execute(deps.as_mut(), creator_env, contract_info, msg).unwrap();
    assert_eq!(execute_res.messages, vec![]);
    assert_eq!(
        execute_res.attributes,
        vec![attr("action", "execute_poll"), attr("poll_id", "1")]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let poll_res: PollResponse = from_binary(&res).unwrap();
    assert_eq!(poll_res.status, PollStatus::Executed);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Executed),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let polls_res: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(polls_res.polls[0], poll_res);
}

#[test]
fn poll_with_none_execute_data_marked_as_executed() {
    const POLL_START_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut creator_env = mock_env_height(POLL_START_HEIGHT, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(POLL_START_HEIGHT, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += DEFAULT_VOTING_PERIOD;

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // End poll will withdraw deposit balance
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(stake_amount as u128),
            )],
        ),
    ]);

    creator_env.block.height += DEFAULT_TIMELOCK_PERIOD;
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env.clone(), creator_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: creator_env.contract.address.to_string(),
                msg: to_binary(&ExecuteMsg::ExecutePollMsgs { poll_id: 1 }).unwrap(),
                funds: vec![],
            }),
            1
        )]
    );

    // Executes since empty polls are allowed
    let msg = ExecuteMsg::ExecutePollMsgs { poll_id: 1 };
    let contract_info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let execute_res = execute(deps.as_mut(), creator_env, contract_info, msg).unwrap();
    assert_eq!(execute_res.messages, vec![]);
    assert_eq!(
        execute_res.attributes,
        vec![attr("action", "execute_poll"), attr("poll_id", "1")]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let poll_res: PollResponse = from_binary(&res).unwrap();
    assert_eq!(poll_res.status, PollStatus::Executed);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Executed),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let polls_res: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(polls_res.polls[0], poll_res);
}

#[test]
fn snapshot_poll() {
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let mut creator_env = mock_env();
    let creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_height", "32345"),
        ]
    );

    //must not be executed
    let snapshot_err = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(ContractError::SnapshotHeight {}, snapshot_err);

    // change time
    creator_env.block.height = 32345 - 10;

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let fix_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap();

    assert_eq!(
        fix_res.attributes,
        vec![
            attr("action", "snapshot_poll"),
            attr("poll_id", "1"),
            attr("staked_amount", stake_amount.to_string().as_str()),
        ]
    );

    //must not be executed
    let snapshot_error = execute(
        deps.as_mut(),
        creator_env,
        creator_info,
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(ContractError::SnapshotAlreadyOccurred {}, snapshot_error);
}

#[test]
fn happy_days_cast_vote_with_snapshot() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(11u128))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(11u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        11,
        DEFAULT_PROPOSAL_DEPOSIT,
        11,
        1,
        execute_res,
        deps.as_ref(),
    );

    //cast_vote without snapshot
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let amount = 10u128;

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    // balance be double
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[
                (&TEST_VOTER.to_string(), &Uint128::from(11_u64)),
                (&TEST_VOTER_2.to_string(), &Uint128::from(11_u64)),
            ],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(22u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(value.staked_amount, None);
    let end_height = value.end_height;

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // another voter cast a vote
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let env = mock_env_height(end_height - 9, 10000);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, amount, 1, VoteOption::Yes, execute_res);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(value.staked_amount, Some(Uint128::new(22)));

    // snanpshot poll will not go through
    let snap_error = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(ContractError::SnapshotAlreadyOccurred {}, snap_error);

    // balance be double
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[
                (&TEST_VOTER.to_string(), &Uint128::from(11_u64)),
                (&TEST_VOTER_2.to_string(), &Uint128::from(11_u64)),
                (&TEST_VOTER_3.to_string(), &Uint128::from(11_u64)),
            ],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(33u128 + DEFAULT_PROPOSAL_DEPOSIT),
            )],
        ),
    ]);

    // another voter cast a vote but the snapshot is already occurred
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_3.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let env = mock_env_height(end_height - 8, 10000);
    let info = mock_info(TEST_VOTER_3, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_3, amount, 1, VoteOption::Yes, execute_res);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(value.staked_amount, Some(Uint128::new(22)));
}

#[test]
fn fails_end_poll_quorum_inflation_without_snapshot_poll() {
    const POLL_START_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let mut creator_env = mock_env_height(POLL_START_HEIGHT, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    //add two messages
    let execute_msgs: Vec<PollExecuteMsg> = vec![
        PollExecuteMsg {
            order: 1u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz.clone(),
        },
        PollExecuteMsg {
            order: 2u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        },
    ];

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(execute_msgs),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(POLL_START_HEIGHT, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_env.block.height += DEFAULT_VOTING_PERIOD - 10;

    // did not SnapshotPoll

    // staked amount get increased 10 times
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[
                (&TEST_VOTER.to_string(), &Uint128::from(2 * stake_amount)),
                (&TEST_VOTER_2.to_string(), &Uint128::from(8 * stake_amount)),
            ],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(((10 * stake_amount) + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(8 * stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // another voter cast a vote
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(creator_env.block.height, 10000);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER_2),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += 10;

    // quorum must reach
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(
        10 * stake_amount,
        value.total_balance_at_end_poll.unwrap().u128()
    );
}

#[test]
fn happy_days_end_poll_with_controlled_quorum() {
    const POLL_START_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());

    let mut creator_env = mock_env_height(POLL_START_HEIGHT, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    //add two messages
    let execute_msgs: Vec<PollExecuteMsg> = vec![
        PollExecuteMsg {
            order: 1u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz.clone(),
        },
        PollExecuteMsg {
            order: 2u64,
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        },
    ];

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(execute_msgs),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env.block.height + DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(stake_amount))],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(POLL_START_HEIGHT, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_env.block.height += DEFAULT_VOTING_PERIOD - 10;

    // send SnapshotPoll
    let fix_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap();

    assert_eq!(
        fix_res.attributes,
        vec![
            attr("action", "snapshot_poll"),
            attr("poll_id", "1"),
            attr("staked_amount", stake_amount.to_string().as_str()),
        ]
    );

    // staked amount get increased 10 times
    deps.querier.with_token_balances(&[
        (
            &VOTING_ESCROW.to_string(),
            &[
                (&TEST_VOTER.to_string(), &Uint128::from(2 * stake_amount)),
                (&TEST_VOTER_2.to_string(), &Uint128::from(8 * stake_amount)),
            ],
        ),
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(((10 * stake_amount) + DEFAULT_PROPOSAL_DEPOSIT) as u128),
            )],
        ),
    ]);

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(8 * stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(8 * stake_amount),
    };
    let env = mock_env_height(creator_env.block.height, 10000);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string().as_str()),
            attr("amount", "8000"),
            attr("voter", TEST_VOTER_2),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.height += 10;

    // quorum must reach
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(
        stake_amount,
        value.total_balance_at_end_poll.unwrap().u128()
    );

    assert_eq!(value.yes_votes.u128(), 9 * stake_amount);

    // actual staked amount is 10 times bigger than staked amount
    let actual_staked_weight = query_token_balance(
        &deps.as_ref().querier,
        Addr::unchecked(VOTING_TOKEN),
        Addr::unchecked(MOCK_CONTRACT_ADDR),
    )
    .unwrap()
    .checked_sub(Uint128::from(DEFAULT_PROPOSAL_DEPOSIT))
    .unwrap();

    assert_eq!(actual_staked_weight.u128(), (10 * stake_amount))
}

#[test]
fn test_migrate() {
    let mut deps = mock_dependencies(&[]);

    let msg = instantiate_msg();
    let info = mock_info(TEST_CREATOR, &coins(2, VOTING_TOKEN));
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let legacy_config: LegacyConfig = read_legacy_config(&deps.storage).unwrap();

    migrate(
        deps.as_mut(),
        mock_env(),
        MigrateMsg {
            anchor_voting_escrow: VOTING_ESCROW.to_string(),
        },
    )
    .unwrap();

    let new_config: Config = config_read(deps.as_ref().storage).load().unwrap();

    assert_eq!(legacy_config.owner, new_config.owner);
    assert_eq!(legacy_config.anchor_token, new_config.anchor_token);
    assert_eq!(legacy_config.quorum, new_config.quorum);
    assert_eq!(legacy_config.threshold, new_config.threshold);
    assert_eq!(legacy_config.voting_period, new_config.voting_period);
    assert_eq!(legacy_config.timelock_period, new_config.timelock_period);
    assert_eq!(
        legacy_config.expiration_period,
        new_config.expiration_period
    );
    assert_eq!(legacy_config.proposal_deposit, new_config.proposal_deposit);
    assert_eq!(legacy_config.snapshot_period, new_config.snapshot_period);
    assert_eq!(
        new_config.anchor_voting_escrow,
        deps.api.addr_canonicalize(VOTING_ESCROW).unwrap()
    );
}

#[test]
fn test_register_contracts() {
    let mut deps = mock_dependencies(&[]);

    let msg = instantiate_msg();
    let info = mock_info(TEST_CREATOR, &coins(2, VOTING_TOKEN));
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let mut config = config_read(&deps.storage).load().unwrap();
    config.anchor_voting_escrow = deps.api.addr_canonicalize("voting-escrow").unwrap();

    config_store(&mut deps.storage).save(&config).unwrap();

    let msg = ExecuteMsg::RegisterContracts {
        anchor_token: "anchor_token".to_string(),
        anchor_voting_escrow: "anchor_voting_escrow".to_string(),
    };

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone());

    match res {
        Err(ContractError::Unauthorized {}) => {}
        _ => panic!("Expected Unauthorized error"),
    }

    config.anchor_voting_escrow = CanonicalAddr::from(vec![]);
    config_store(&mut deps.storage).save(&config).unwrap();

    let _res = execute(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles RegisterContracts");
}
