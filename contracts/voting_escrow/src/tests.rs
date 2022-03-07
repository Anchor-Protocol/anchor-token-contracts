use crate::contract::{instantiate, query};
use anchor_token::voting_escrow::{
    ConfigResponse, InstantiateMarketingInfo, InstantiateMsg, QueryMsg,
};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_binary, Uint128};
use cw20::{Logo, LogoInfo, MarketingInfoResponse, TokenInfoResponse};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor".to_string(),
        marketing: Some(InstantiateMarketingInfo {
            project: Some("voted-escrow".to_string()),
            description: Some("voted-escrow".to_string()),
            logo: Some(Logo::Url("votes-escrow-url".to_string())),
            marketing: Some("marketing".to_string()),
        }),
    };

    let info = mock_info("owner", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();

    assert_eq!(config.owner, "owner".to_string());
    assert_eq!(config.anchor_token, "anchor".to_string());

    let res = query(deps.as_ref(), mock_env(), QueryMsg::MarketingInfo {}).unwrap();
    let marketing: MarketingInfoResponse = from_binary(&res).unwrap();

    assert_eq!(marketing.project.unwrap(), "voted-escrow".to_string());
    assert_eq!(marketing.description.unwrap(), "voted-escrow".to_string());
    assert_eq!(
        marketing.logo.unwrap(),
        LogoInfo::Url("votes-escrow-url".to_string())
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();

    assert_eq!(token_info.name, "veANC".to_string());
    assert_eq!(token_info.symbol, "veANC".to_string());
    assert_eq!(token_info.decimals, 6);
    assert_eq!(token_info.total_supply, Uint128::zero());
}
