use crate::contract::{handle, init, query};
use anchor_token::staking::{
    ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, QueryMsg, StakerInfoResponse, StateResponse,
};
use cosmwasm_std::testing::{mock_dependencies, mock_env};
use cosmwasm_std::{
    from_binary, to_binary, CosmosMsg, Decimal, HumanAddr, StdError, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        anchor_token: HumanAddr("reward0000".to_string()),
        staking_token: HumanAddr("staking0000".to_string()),
        distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            anchor_token: HumanAddr::from("reward0000"),
            staking_token: HumanAddr::from("staking0000"),
            distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
        }
    );

    let res = query(&deps, QueryMsg::State { block_height: None }).unwrap();
    let state: StateResponse = from_binary(&res).unwrap();
    assert_eq!(
        state,
        StateResponse {
            last_distributed: 12345,
            total_bond_amount: Uint128::zero(),
            global_reward_index: Decimal::zero(),
        }
    );
}

#[test]
fn test_bond_tokens() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        anchor_token: HumanAddr("reward0000".to_string()),
        staking_token: HumanAddr("staking0000".to_string()),
        distribution_schedule: vec![
            (12345, 12345 + 100, Uint128::from(1000000u128)),
            (12345 + 100, 12345 + 200, Uint128::from(10000000u128)),
        ],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });

    let mut env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                &deps,
                QueryMsg::StakerInfo {
                    staker: HumanAddr::from("addr0000"),
                    block_height: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            bond_amount: Uint128(100u128),
        }
    );

    assert_eq!(
        from_binary::<StateResponse>(
            &query(&deps, QueryMsg::State { block_height: None }).unwrap()
        )
        .unwrap(),
        StateResponse {
            total_bond_amount: Uint128(100u128),
            global_reward_index: Decimal::zero(),
            last_distributed: 12345,
        }
    );

    // bond 100 more tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    env.block.height += 10;

    let _res = handle(&mut deps, env, msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                &deps,
                QueryMsg::StakerInfo {
                    staker: HumanAddr::from("addr0000"),
                    block_height: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_index: Decimal::from_ratio(1000u128, 1u128),
            pending_reward: Uint128::from(100000u128),
            bond_amount: Uint128(200u128),
        }
    );

    assert_eq!(
        from_binary::<StateResponse>(
            &query(&deps, QueryMsg::State { block_height: None }).unwrap()
        )
        .unwrap(),
        StateResponse {
            total_bond_amount: Uint128(200u128),
            global_reward_index: Decimal::from_ratio(1000u128, 1u128),
            last_distributed: 12345 + 10,
        }
    );

    // failed with unautorized
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });

    let env = mock_env("staking0001", &[]);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_unbond() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        anchor_token: HumanAddr("reward0000".to_string()),
        staking_token: HumanAddr("staking0000".to_string()),
        distribution_schedule: vec![
            (12345, 12345 + 100, Uint128::from(1000000u128)),
            (12345 + 100, 12345 + 200, Uint128::from(10000000u128)),
        ],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // unbond 150 tokens; failed
    let msg = HandleMsg::Unbond {
        amount: Uint128(150u128),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot unbond more than bond amount");
        }
        _ => panic!("Must return generic error"),
    };

    // normal unbond
    let msg = HandleMsg::Unbond {
        amount: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("staking0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128(100u128),
            })
            .unwrap(),
            send: vec![],
        })]
    );
}

#[test]
fn test_compute_reward() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        anchor_token: HumanAddr("reward0000".to_string()),
        staking_token: HumanAddr("staking0000".to_string()),
        distribution_schedule: vec![
            (12345, 12345 + 100, Uint128::from(1000000u128)),
            (12345 + 100, 12345 + 200, Uint128::from(10000000u128)),
        ],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let mut env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // 100 blocks passed
    // 1,000,000 rewards distributed
    env.block.height += 100;

    // bond 100 more tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                &deps,
                QueryMsg::StakerInfo {
                    staker: HumanAddr::from("addr0000"),
                    block_height: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_index: Decimal::from_ratio(10000u128, 1u128),
            pending_reward: Uint128(1000000u128),
            bond_amount: Uint128(200u128),
        }
    );

    // 100 blocks passed
    // 1,000,000 rewards distributed
    env.block.height += 10;
    env.message.sender = HumanAddr::from("addr0000");

    // unbond
    let msg = HandleMsg::Unbond {
        amount: Uint128(100u128),
    };
    let _res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                &deps,
                QueryMsg::StakerInfo {
                    staker: HumanAddr::from("addr0000"),
                    block_height: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_index: Decimal::from_ratio(15000u64, 1u64),
            pending_reward: Uint128(2000000u128),
            bond_amount: Uint128(100u128),
        }
    );

    // query future block
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                &deps,
                QueryMsg::StakerInfo {
                    staker: HumanAddr::from("addr0000"),
                    block_height: Some(12345 + 120),
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_index: Decimal::from_ratio(25000u64, 1u64),
            pending_reward: Uint128(3000000u128),
            bond_amount: Uint128(100u128),
        }
    );
}

#[test]
fn test_withdraw() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        anchor_token: HumanAddr("reward0000".to_string()),
        staking_token: HumanAddr("staking0000".to_string()),
        distribution_schedule: vec![
            (12345, 12345 + 100, Uint128::from(1000000u128)),
            (12345 + 100, 12345 + 200, Uint128::from(10000000u128)),
        ],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let mut env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // 100 blocks passed
    // 1,000,000 rewards distributed
    env.block.height += 100;
    env.message.sender = HumanAddr::from("addr0000");

    let msg = HandleMsg::Withdraw {};
    let res = handle(&mut deps, env, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("reward0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128(1000000u128),
            })
            .unwrap(),
            send: vec![],
        })]
    );
}
