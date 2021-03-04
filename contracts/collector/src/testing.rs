use crate::contract::{handle, init, query_config};
use crate::mock_querier::mock_dependencies;
use anchor_token::collector::{ConfigResponse, HandleMsg, InitMsg};
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{to_binary, Coin, CosmosMsg, Decimal, HumanAddr, StdError, Uint128, WasmMsg};
use cw20::Cw20HandleMsg;
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::HandleMsg as TerraswapHandleMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        gov_contract: HumanAddr("gov".to_string()),
        anchor_token: HumanAddr("tokenANC".to_string()),
        distributor_contract: HumanAddr::from("distributor"),
        reward_factor: Decimal::percent(90),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse = query_config(&deps).unwrap();
    assert_eq!("terraswapfactory", config.terraswap_factory.as_str());
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        gov_contract: HumanAddr("gov".to_string()),
        anchor_token: HumanAddr("tokenANC".to_string()),
        distributor_contract: HumanAddr::from("distributor"),
        reward_factor: Decimal::percent(90),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // update reward_factor
    let env = mock_env("gov", &[]);
    let msg = HandleMsg::UpdateConfig {
        reward_factor: Some(Decimal::percent(80)),
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let value = query_config(&deps).unwrap();
    assert_eq!(Decimal::percent(80), value.reward_factor);

    // Unauthorized err
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        reward_factor: None,
    };

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_sweep() {
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128),
        }],
    );

    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );

    deps.querier
        .with_terraswap_pairs(&[(&"uusdtokenANC".to_string(), &HumanAddr::from("pairANC"))]);

    let msg = InitMsg {
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        gov_contract: HumanAddr("gov".to_string()),
        anchor_token: HumanAddr("tokenANC".to_string()),
        distributor_contract: HumanAddr::from("distributor"),
        reward_factor: Decimal::percent(90),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Sweep {
        denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();

    // tax deduct 100 => 99
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("pairANC"),
                msg: to_binary(&TerraswapHandleMsg::Swap {
                    offer_asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uusd".to_string()
                        },
                        amount: Uint128::from(99u128),
                    },
                    max_spread: None,
                    belief_price: None,
                    to: None,
                })
                .unwrap(),
                send: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(99u128),
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                msg: to_binary(&HandleMsg::Distribute {}).unwrap(),
                send: vec![],
            })
        ]
    );
}

#[test]
fn test_distribute() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("tokenANC"),
        &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100u128))],
    )]);

    let msg = InitMsg {
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        gov_contract: HumanAddr("gov".to_string()),
        anchor_token: HumanAddr("tokenANC".to_string()),
        distributor_contract: HumanAddr::from("distributor"),
        reward_factor: Decimal::percent(90),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();
    let msg = HandleMsg::Distribute {};

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env(MOCK_CONTRACT_ADDR, &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("tokenANC"),
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("gov"),
                    amount: Uint128(90u128),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("tokenANC"),
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("distributor"),
                    amount: Uint128(10u128),
                })
                .unwrap(),
                send: vec![],
            })
        ]
    )
}
