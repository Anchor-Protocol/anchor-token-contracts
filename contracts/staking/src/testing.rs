use crate::contract::{execute, instantiate, query};
use crate::mock_querier::mock_dependencies;
use anchor_token::staking::ExecuteMsg::UpdateConfig;
use anchor_token::staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StakerInfoResponse,
    StateResponse,
};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{
    attr, from_binary, to_binary, CosmosMsg, Decimal, StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        anchor_token: "reward0000".to_string(),
        staking_token: "staking0000".to_string(),
        distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            anchor_token: "reward0000".to_string(),
            staking_token: "staking0000".to_string(),
            distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::State { block_time: None },
    )
    .unwrap();
    let state: StateResponse = from_binary(&res).unwrap();
    assert_eq!(
        state,
        StateResponse {
            last_distributed: mock_env().block.time.seconds(),
            total_bond_amount: Uint128::zero(),
            global_reward_index: Decimal::zero(),
        }
    );
}

#[test]
fn test_bond_tokens() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        anchor_token: "reward0000".to_string(),
        staking_token: "staking0000".to_string(),
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("staking0000", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            bond_amount: Uint128::from(100u128),
        }
    );

    assert_eq!(
        from_binary::<StateResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::State { block_time: None }
            )
            .unwrap()
        )
        .unwrap(),
        StateResponse {
            total_bond_amount: Uint128::from(100u128),
            global_reward_index: Decimal::zero(),
            last_distributed: mock_env().block.time.seconds(),
        }
    );

    // bond 100 more tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    env.block.time = env.block.time.plus_seconds(10);

    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal::from_ratio(1000u128, 1u128),
            pending_reward: Uint128::from(100000u128),
            bond_amount: Uint128::from(200u128),
        }
    );

    assert_eq!(
        from_binary::<StateResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::State { block_time: None }
            )
            .unwrap()
        )
        .unwrap(),
        StateResponse {
            total_bond_amount: Uint128::from(200u128),
            global_reward_index: Decimal::from_ratio(1000u128, 1u128),
            last_distributed: mock_env().block.time.seconds() + 10,
        }
    );

    // failed with unautorized
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("staking0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_unbond() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        anchor_token: "reward0000".to_string(),
        staking_token: "staking0000".to_string(),
        distribution_schedule: vec![
            (12345, 12345 + 100, Uint128::from(1000000u128)),
            (12345 + 100, 12345 + 200, Uint128::from(10000000u128)),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("staking0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // unbond 150 tokens; failed
    let msg = ExecuteMsg::Unbond {
        amount: Uint128::from(150u128),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot unbond more than bond amount");
        }
        _ => panic!("Must return generic error"),
    };

    // normal unbond
    let msg = ExecuteMsg::Unbond {
        amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "staking0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn test_compute_reward() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        anchor_token: "reward0000".to_string(),
        staking_token: "staking0000".to_string(),
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("staking0000", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // 100 seconds passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);

    // bond 100 more tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal::from_ratio(10000u128, 1u128),
            pending_reward: Uint128::from(1000000u128),
            bond_amount: Uint128::from(200u128),
        }
    );

    // 100 seconds passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(10);
    let info = mock_info("addr0000", &[]);

    // unbond
    let msg = ExecuteMsg::Unbond {
        amount: Uint128::from(100u128),
    };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal::from_ratio(15000u64, 1u64),
            pending_reward: Uint128::from(2000000u128),
            bond_amount: Uint128::from(100u128),
        }
    );

    // query future block
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: Some(mock_env().block.time.plus_seconds(120).seconds()),
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal::from_ratio(25000u64, 1u64),
            pending_reward: Uint128::from(3000000u128),
            bond_amount: Uint128::from(100u128),
        }
    );
}

#[test]
fn test_withdraw() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        anchor_token: "reward0000".to_string(),
        staking_token: "staking0000".to_string(),
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("staking0000", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // 100 seconds passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);

    let info = mock_info("addr0000", &[]);

    let msg = ExecuteMsg::Withdraw {};
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "reward0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(1000000u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn test_migrate_staking() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        anchor_token: "reward0000".to_string(),
        staking_token: "staking0000".to_string(),
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("staking0000", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // 100 seconds is passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);
    let info = mock_info("addr0000", &[]);

    let msg = ExecuteMsg::Withdraw {};
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "reward0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(1000000u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // execute migration after 50 seconds
    env.block.time = env.block.time.plus_seconds(50);

    deps.querier.with_anc_minter("gov0000".to_string());

    let msg = ExecuteMsg::MigrateStaking {
        new_staking_contract: "newstaking0000".to_string(),
    };

    // unauthorized attempt
    let info = mock_info("notgov0000", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }

    // successful attempt
    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "migrate_staking"),
            attr("distributed_amount", "6000000"), // 1000000 + (10000000 / 2)
            attr("remaining_amount", "5000000")    // 11,000,000 - 6000000
        ]
    );

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "reward0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "newstaking0000".to_string(),
                amount: Uint128::from(5000000u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // query config
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            anchor_token: "reward0000".to_string(),
            staking_token: "staking0000".to_string(),
            distribution_schedule: vec![
                (
                    mock_env().block.time.seconds(),
                    mock_env().block.time.seconds() + 100,
                    Uint128::from(1000000u128)
                ),
                (
                    mock_env().block.time.seconds() + 100,
                    mock_env().block.time.seconds() + 150,
                    Uint128::from(5000000u128)
                ), // slot was modified
            ]
        }
    );
}

#[test]
fn test_update_global_index() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        anchor_token: "reward0000".to_string(),
        staking_token: "staking0000".to_string(),
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 200,
                mock_env().block.time.seconds() + 300,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 300,
                mock_env().block.time.seconds() + 400,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 400,
                mock_env().block.time.seconds() + 500,
                Uint128::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let update_config = UpdateConfig {
        distribution_schedule: vec![(
            mock_env().block.time.seconds() + 300,
            mock_env().block.time.seconds() + 400,
            Uint128::from(10000000u128),
        )],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("notgov", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }

    //update the overlapped schedule
    let update_config = UpdateConfig {
        distribution_schedule: vec![(
            mock_env().block.time.seconds() + 250,
            mock_env().block.time.seconds() + 300,
            Uint128::from(10000000u128),
        )],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "cannot update the overlapped distribution")
        }
        _ => panic!("Must return unauthorized error"),
    }

    //update the overlapped schedule
    let update_config = UpdateConfig {
        distribution_schedule: vec![(
            mock_env().block.time.seconds() + 250,
            mock_env().block.time.seconds() + 299,
            Uint128::from(10000000u128),
        )],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "cannot update the overlapped distribution")
        }
        _ => panic!("Must return unauthorized error"),
    }
    // do some bond and update rewards
    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("staking0000", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // 100 seconds is passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);
    let info = mock_info("addr0000", &[]);

    let msg = ExecuteMsg::Withdraw {};
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "reward0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(1000000u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    let update_config = UpdateConfig {
        distribution_schedule: vec![(
            mock_env().block.time.seconds(),
            mock_env().block.time.seconds() + 100,
            Uint128::from(10000000u128),
        )],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "cannot update the ongoing schedule")
        }
        _ => panic!("Must return unauthorized error"),
    }

    // do some bond and update rewards
    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("staking0000", &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // 100 seconds is passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);

    let info = mock_info("addr0000", &[]);

    let msg = ExecuteMsg::Withdraw {};
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    //cannot update previous scehdule
    let update_config = UpdateConfig {
        distribution_schedule: vec![(
            mock_env().block.time.seconds(),
            mock_env().block.time.seconds() + 100,
            Uint128::from(10000000u128),
        )],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "cannot update a previous schedule")
        }
        _ => panic!("Must return unauthorized error"),
    }

    //successful one
    let update_config = UpdateConfig {
        distribution_schedule: vec![(
            mock_env().block.time.seconds() + 300,
            mock_env().block.time.seconds() + 400,
            Uint128::from(20000000u128),
        )],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config).unwrap();

    assert_eq!(res.attributes, vec![("action", "update_config")]);

    // query config
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config.distribution_schedule,
        vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 200,
                mock_env().block.time.seconds() + 300,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 300,
                mock_env().block.time.seconds() + 400,
                Uint128::from(20000000u128),
            ),
            (
                mock_env().block.time.seconds() + 400,
                mock_env().block.time.seconds() + 500,
                Uint128::from(10000000u128),
            ),
        ]
    );

    //successful one
    let update_config = UpdateConfig {
        distribution_schedule: vec![(
            mock_env().block.time.seconds() + 400,
            mock_env().block.time.seconds() + 500,
            Uint128::from(50000000u128),
        )],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config).unwrap();

    assert_eq!(res.attributes, vec![("action", "update_config")]);

    // query config
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config.distribution_schedule,
        vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 200,
                mock_env().block.time.seconds() + 300,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 300,
                mock_env().block.time.seconds() + 400,
                Uint128::from(20000000u128),
            ),
            (
                mock_env().block.time.seconds() + 400,
                mock_env().block.time.seconds() + 500,
                Uint128::from(50000000u128),
            ),
        ]
    );

    let update_config = UpdateConfig {
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds() + 300,
                mock_env().block.time.seconds() + 400,
                Uint128::from(90000000u128),
            ),
            (
                mock_env().block.time.seconds() + 400,
                mock_env().block.time.seconds() + 500,
                Uint128::from(80000000u128),
            ),
        ],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config).unwrap();

    assert_eq!(res.attributes, vec![("action", "update_config")]);

    // query config
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config.distribution_schedule,
        vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 200,
                mock_env().block.time.seconds() + 300,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 300,
                mock_env().block.time.seconds() + 400,
                Uint128::from(90000000u128),
            ),
            (
                mock_env().block.time.seconds() + 400,
                mock_env().block.time.seconds() + 500,
                Uint128::from(80000000u128),
            ),
        ]
    );

    let update_config = UpdateConfig {
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds() + 300,
                mock_env().block.time.seconds() + 400,
                Uint128::from(90000000u128),
            ),
            (
                mock_env().block.time.seconds() + 400,
                mock_env().block.time.seconds() + 500,
                Uint128::from(80000000u128),
            ),
            (
                mock_env().block.time.seconds() + 500,
                mock_env().block.time.seconds() + 600,
                Uint128::from(60000000u128),
            ),
        ],
    };

    deps.querier.with_anc_minter("gov0000".to_string());

    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, update_config).unwrap();

    assert_eq!(res.attributes, vec![("action", "update_config")]);

    // query config
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config.distribution_schedule,
        vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 200,
                mock_env().block.time.seconds() + 300,
                Uint128::from(10000000u128),
            ),
            (
                mock_env().block.time.seconds() + 300,
                mock_env().block.time.seconds() + 400,
                Uint128::from(90000000u128),
            ),
            (
                mock_env().block.time.seconds() + 400,
                mock_env().block.time.seconds() + 500,
                Uint128::from(80000000u128),
            ),
            (
                mock_env().block.time.seconds() + 500,
                mock_env().block.time.seconds() + 600,
                Uint128::from(60000000u128),
            )
        ]
    );
}
