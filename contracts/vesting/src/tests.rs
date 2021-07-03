use crate::contract::{execute, instantiate, query};
use anchor_token::common::OrderBy;
use anchor_token::vesting::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, VestingAccount, VestingAccountResponse,
    VestingAccountsResponse, VestingInfo,
};

use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{
    from_binary, attr, to_binary, CosmosMsg, StdError, SubMsg, Timestamp, Uint128, WasmMsg
};
use cw20::Cw20ExecuteMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor_token".to_string(),
        genesis_time: 12345u64,
    };

    let info = mock_info("addr0000", &vec![]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        from_binary::<ConfigResponse>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap(),
        ConfigResponse {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            genesis_time: 12345u64,
        }
    );
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor_token".to_string(),
        genesis_time: 12345u64,
    };

    let info = mock_info("addr0000", &vec![]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner2".to_string()),
        anchor_token: None,
        genesis_time: None,
    };
    let info = mock_info("owner", &vec![]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        from_binary::<ConfigResponse>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap(),
        ConfigResponse {
            owner: "owner2".to_string(),
            anchor_token: "anchor_token".to_string(),
            genesis_time: 12345u64,
        }
    );

    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner".to_string()),
        anchor_token: None,
        genesis_time: None,
    };
    let info = mock_info("owner", &vec![]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        anchor_token: Some("anchor_token2".to_string()),
        genesis_time: Some(1u64),
    };
    let info = mock_info("owner2", &vec![]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        from_binary::<ConfigResponse>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap(),
        ConfigResponse {
            owner: "owner2".to_string(),
            anchor_token: "anchor_token2".to_string(),
            genesis_time: 1u64,
        }
    );
}

#[test]
fn register_vesting_accounts() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor_token".to_string(),
        genesis_time: 100u64,
    };

    let info = mock_info("addr0000", &vec![]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterVestingAccounts {
        vesting_accounts: vec![
            VestingAccount {
                address: "addr0000".to_string(),
                schedules: vec![
                    (100u64, 101u64, Uint128::from(100u128)),
                    (100u64, 110u64, Uint128::from(100u128)),
                    (100u64, 200u64, Uint128::from(100u128)),
                ],
            },
            VestingAccount {
                address: "addr0001".to_string(),
                schedules: vec![(100u64, 110u64, Uint128::from(100u128))],
            },
            VestingAccount {
                address: "addr0002".to_string(),
                schedules: vec![(100u64, 200u64, Uint128::from(100u128))],
            },
        ],
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        from_binary::<VestingAccountResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::VestingAccount {
                    address: "addr0000".to_string(),
                }
            )
            .unwrap()
        )
        .unwrap(),
        VestingAccountResponse {
            address: "addr0000".to_string(),
            info: VestingInfo {
                last_claim_time: 100u64,
                schedules: vec![
                    (100u64, 101u64, Uint128::from(100u128)),
                    (100u64, 110u64, Uint128::from(100u128)),
                    (100u64, 200u64, Uint128::from(100u128)),
                ],
            }
        }
    );

    assert_eq!(
        from_binary::<VestingAccountsResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::VestingAccounts {
                    limit: None,
                    start_after: None,
                    order_by: Some(OrderBy::Asc),
                }
            )
            .unwrap()
        )
        .unwrap(),
        VestingAccountsResponse {
            vesting_accounts: vec![
                VestingAccountResponse {
                    address: "addr0000".to_string(),
                    info: VestingInfo {
                        last_claim_time: 100u64,
                        schedules: vec![
                            (100u64, 101u64, Uint128::from(100u128)),
                            (100u64, 110u64, Uint128::from(100u128)),
                            (100u64, 200u64, Uint128::from(100u128)),
                        ],
                    }
                },
                VestingAccountResponse {
                    address: "addr0001".to_string(),
                    info: VestingInfo {
                        last_claim_time: 100u64,
                        schedules: vec![(100u64, 110u64, Uint128::from(100u128))],
                    }
                },
                VestingAccountResponse {
                    address: "addr0002".to_string(),
                    info: VestingInfo {
                        last_claim_time: 100u64,
                        schedules: vec![(100u64, 200u64, Uint128::from(100u128))],
                    }
                }
            ]
        }
    );
}

#[test]
fn claim() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor_token".to_string(),
        genesis_time: 100u64,
    };

    let info = mock_info("addr0000", &vec![]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterVestingAccounts {
        vesting_accounts: vec![VestingAccount {
            address: "addr0000".to_string(),
            schedules: vec![
                (100u64, 101u64, Uint128::from(100u128)),
                (100u64, 110u64, Uint128::from(100u128)),
                (100u64, 200u64, Uint128::from(100u128)),
            ],
        }],
    };
    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    let info = mock_info("addr0000", &[]);
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(100);

    let msg = ExecuteMsg::Claim {};
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim"),
            attr("address", "addr0000"),
            attr("claim_amount", "0"),
            attr("last_claim_time", "100"),
        ]
    );
    assert_eq!(res.messages, vec![]);

    env.block.time = Timestamp::from_seconds(101);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim"),
            attr("address", "addr0000"),
            attr("claim_amount", "111"),
            attr("last_claim_time", "101"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anchor_token".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(111u128), // TODO: wtf
            })
            .unwrap(),
            funds: vec![],
        }))],
    );

    env.block.time = Timestamp::from_seconds(102);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim"),
            attr("address", "addr0000"),
            attr("claim_amount", "11"),
            attr("last_claim_time", "102"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anchor_token".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(11u128), // TODO: wtf
            })
            .unwrap(),
            funds: vec![],
        }))],
    );
}
