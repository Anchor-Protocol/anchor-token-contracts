use crate::contract::{execute, instantiate, query};

use anchor_token::distributor::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_binary, to_binary, CosmosMsg, StdError, SubMsg, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        gov_contract: "gov".to_string(),
        anchor_token: "anchor".to_string(),
        whitelist: vec![
            "addr1".to_string(),
            "addr2".to_string(),
            "addr3".to_string(),
        ],
        spend_limit: Uint128::from(1000000u128),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!("gov", config.gov_contract.as_str());
    assert_eq!("anchor", config.anchor_token.as_str());
    assert_eq!(
        vec![
            "addr1".to_string(),
            "addr2".to_string(),
            "addr3".to_string(),
        ],
        config.whitelist
    );
    assert_eq!(Uint128::from(1000000u128), config.spend_limit);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        gov_contract: "gov".to_string(),
        anchor_token: "anchor".to_string(),
        whitelist: vec![
            "addr1".to_string(),
            "addr2".to_string(),
            "addr3".to_string(),
        ],
        spend_limit: Uint128::from(1000000u128),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!("gov", config.gov_contract.as_str());
    assert_eq!("anchor", config.anchor_token.as_str());
    assert_eq!(
        vec![
            "addr1".to_string(),
            "addr2".to_string(),
            "addr3".to_string(),
        ],
        config.whitelist
    );
    assert_eq!(Uint128::from(1000000u128), config.spend_limit);

    let msg = ExecuteMsg::UpdateConfig {
        spend_limit: Some(Uint128::from(500000u128)),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());

    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("gov", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            gov_contract: "gov".to_string(),
            anchor_token: "anchor".to_string(),
            whitelist: vec![
                "addr1".to_string(),
                "addr2".to_string(),
                "addr3".to_string(),
            ],
            spend_limit: Uint128::from(500000u128),
        }
    );
}

#[test]
fn test_add_remove_distributor() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        gov_contract: "gov".to_string(),
        anchor_token: "anchor".to_string(),
        whitelist: vec![
            "addr1".to_string(),
            "addr2".to_string(),
            "addr3".to_string(),
        ],
        spend_limit: Uint128::from(1000000u128),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Permission check AddDistributor
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::AddDistributor {
        distributor: "addr4".to_string(),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // Permission check RemoveDistributor
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::RemoveDistributor {
        distributor: "addr4".to_string(),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // AddDistributor
    let info = mock_info("gov", &[]);
    let msg = ExecuteMsg::AddDistributor {
        distributor: "addr4".to_string(),
    };

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            gov_contract: "gov".to_string(),
            anchor_token: "anchor".to_string(),
            whitelist: vec![
                "addr1".to_string(),
                "addr2".to_string(),
                "addr3".to_string(),
                "addr4".to_string(),
            ],
            spend_limit: Uint128::from(1000000u128),
        }
    );

    // RemoveDistributor
    let info = mock_info("gov", &[]);
    let msg = ExecuteMsg::RemoveDistributor {
        distributor: "addr1".to_string(),
    };

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            gov_contract: "gov".to_string(),
            anchor_token: "anchor".to_string(),
            whitelist: vec![
                "addr2".to_string(),
                "addr3".to_string(),
                "addr4".to_string(),
            ],
            spend_limit: Uint128::from(1000000u128),
        }
    );
}

#[test]
fn test_spend() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        gov_contract: "gov".to_string(),
        anchor_token: "anchor".to_string(),
        whitelist: vec![
            "addr1".to_string(),
            "addr2".to_string(),
            "addr3".to_string(),
        ],
        spend_limit: Uint128::from(1000000u128),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // permission failed
    let msg = ExecuteMsg::Spend {
        recipient: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // failed due to spend limit
    let msg = ExecuteMsg::Spend {
        recipient: "addr0000".to_string(),
        amount: Uint128::from(2000000u128),
    };

    let info = mock_info("addr1", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot spend more than spend_limit")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::Spend {
        recipient: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    };

    let info = mock_info("addr2", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anchor".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(1000000u128),
            })
            .unwrap(),
        }))]
    );
}
