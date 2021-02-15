use crate::contract::{handle, init, query};

use anchor_token::community::{ConfigResponse, HandleMsg, InitMsg, QueryMsg};
use cosmwasm_std::testing::{mock_dependencies, mock_env};
use cosmwasm_std::{from_binary, to_binary, CosmosMsg, HumanAddr, StdError, Uint128, WasmMsg};
use cw20::Cw20HandleMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        gov_contract: HumanAddr("gov".to_string()),
        anchor_token: HumanAddr("anchor".to_string()),
        spend_limit: Uint128::from(1000000u128),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse = from_binary(&query(&deps, QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!("gov", config.gov_contract.as_str());
    assert_eq!("anchor", config.anchor_token.as_str());
    assert_eq!(Uint128::from(1000000u128), config.spend_limit);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        gov_contract: HumanAddr("gov".to_string()),
        anchor_token: HumanAddr("anchor".to_string()),
        spend_limit: Uint128::from(1000000u128),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse = from_binary(&query(&deps, QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!("gov", config.gov_contract.as_str());
    assert_eq!("anchor", config.anchor_token.as_str());
    assert_eq!(Uint128::from(1000000u128), config.spend_limit);

    let msg = HandleMsg::UpdateConfig {
        spend_limit: Some(Uint128::from(500000u128)),
    };
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone());

    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("gov", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let config: ConfigResponse = from_binary(&query(&deps, QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            gov_contract: HumanAddr::from("gov"),
            anchor_token: HumanAddr::from("anchor"),
            spend_limit: Uint128::from(500000u128),
        }
    );
}

#[test]
fn test_spend() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        gov_contract: HumanAddr("gov".to_string()),
        anchor_token: HumanAddr("anchor".to_string()),
        spend_limit: Uint128::from(1000000u128),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // permission failed
    let msg = HandleMsg::Spend {
        recipient: HumanAddr::from("addr0000"),
        amount: Uint128::from(1000000u128),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    // failed due to spend limit
    let msg = HandleMsg::Spend {
        recipient: HumanAddr::from("addr0000"),
        amount: Uint128::from(2000000u128),
    };

    let env = mock_env("gov", &[]);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot spend more than spend_limit")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Spend {
        recipient: HumanAddr::from("addr0000"),
        amount: Uint128::from(1000000u128),
    };

    let env = mock_env("gov", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("anchor"),
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(1000000u128),
            })
            .unwrap(),
        })]
    );
}
