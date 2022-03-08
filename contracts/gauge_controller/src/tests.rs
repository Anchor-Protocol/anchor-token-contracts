use crate::error::ContractError;

use cosmwasm_std::{from_binary, Uint128};

use crate::contract::{execute, instantiate, query};
use anchor_token::gauge_controller::{
    AllGaugeAddrResponse, ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeCountResponse,
    GaugeWeightResponse, InstantiateMsg, QueryMsg,
};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

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

// test AddGauge, GaugeCount, GaugeWeight, GaugeAddr, AllGaugeAddr
#[test]
fn test_add_two_gauges() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor_token".to_string(),
        anchor_voting_escorw: "anchor_voting_escrow".to_string(),
    };
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::AddGauge {
        addr: "gauge_addr_1".to_string(),
        weight: Uint128::from(100_u64),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());

    match res {
        Ok(_) => (),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let res: GaugeCountResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::GaugeCount {}).unwrap()).unwrap();

    assert_eq!(1, res.gauge_count);

    let res: GaugeWeightResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GaugeWeight {
                addr: "gauge_addr_1".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(Uint128::from(100_u64), res.gauge_weight);

    let res: GaugeAddrResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GaugeAddr { gauge_id: 0_u64 },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!("gauge_addr_1".to_string(), res.gauge_addr);

    match query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::GaugeAddr { gauge_id: 1_u64 },
    ) {
        Err(ContractError::GaugeNotFound {}) => (),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());

    match res {
        Err(ContractError::GaugeAlreadyExist {}) => (),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::AddGauge {
        addr: "gauge_addr_2".to_string(),
        weight: Uint128::from(200_u64),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());

    match res {
        Ok(_) => (),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let res: GaugeCountResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::GaugeCount {}).unwrap()).unwrap();

    assert_eq!(2, res.gauge_count);

    let all_gauge_addr: AllGaugeAddrResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::AllGaugeAddr {}).unwrap()).unwrap();

    assert_eq!(
        vec!["gauge_addr_1".to_string(), "gauge_addr_2".to_string()],
        all_gauge_addr.all_gauge_addr
    );
}
