#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Reply,
    Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};

use crate::state::{read_config, store_config, Config};

use crate::migration::migrate_config;
use anchor_token::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cw20::Cw20ExecuteMsg;
use std::str::FromStr;
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::ExecuteMsg as TerraswapExecuteMsg;
use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    store_config(
        deps.storage,
        &Config {
            gov_contract: deps.api.addr_canonicalize(&msg.gov_contract)?,
            terraswap_factory: deps.api.addr_canonicalize(&msg.terraswap_factory)?,
            anchor_token: deps.api.addr_canonicalize(&msg.anchor_token)?,
            reward_factor: msg.reward_factor,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { reward_factor } => update_config(deps, info, reward_factor),
        ExecuteMsg::Sweep { denom } => sweep(deps, env, denom),
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    reward_factor: Option<Decimal256>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;
    if deps.api.addr_canonicalize(info.sender.as_str())? != config.gov_contract {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(reward_factor) = reward_factor {
        config.reward_factor = reward_factor;
    }

    store_config(deps.storage, &config)?;
    Ok(Response::default())
}

const SWEEP_REPLY_ID: u64 = 1;

/// Sweep
/// Anyone can execute sweep function to swap
/// asset token => ANC token and distribute
/// result ANC token to gov contract
pub fn sweep(deps: DepsMut, env: Env, denom: String) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let anchor_token = deps.api.addr_humanize(&config.anchor_token)?;
    let terraswap_factory_addr = deps.api.addr_humanize(&config.terraswap_factory)?;

    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        terraswap_factory_addr,
        &[
            AssetInfo::NativeToken {
                denom: denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: anchor_token.to_string(),
            },
        ],
    )?;

    let amount = query_balance(&deps.querier, env.contract.address, denom.to_string())?;

    let swap_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: denom.to_string(),
        },
        amount,
    };

    // deduct tax first
    let amount = (swap_asset.deduct_tax(&deps.querier)?).amount;
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_info.contract_addr,
                msg: to_binary(&TerraswapExecuteMsg::Swap {
                    offer_asset: Asset {
                        amount,
                        ..swap_asset
                    },
                    max_spread: None,
                    belief_price: None,
                    to: None,
                })?,
                funds: vec![Coin {
                    denom: denom.to_string(),
                    amount,
                }],
            }),
            SWEEP_REPLY_ID,
        ))
        .add_attributes(vec![
            attr("action", "sweep"),
            attr(
                "collected_rewards",
                format!("{:?}{:?}", amount.to_string(), denom),
            ),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    if msg.id == SWEEP_REPLY_ID {
        // send tokens on successful callback
        return distribute(deps, env);
    }

    Err(StdError::generic_err("not supported reply"))
}

// Only contract itself can execute distribute function
pub fn distribute(deps: DepsMut, env: Env) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let amount = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.anchor_token)?,
        env.contract.address,
    )?;

    // make decimal256 multiplication work
    let decimal_amount: Decimal256 = Decimal::from_ratio(amount, Uint128::new(1u128)).into();
    let distributed_amount_decimals: Decimal256 = decimal_amount * config.reward_factor;
    let distribute_amount = Uint256::from_str(&distributed_amount_decimals.to_string()).unwrap();

    let left_amount = amount.checked_sub(distribute_amount.into())?;

    let mut messages: Vec<CosmosMsg> = vec![];

    if !distribute_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.anchor_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: deps.api.addr_humanize(&config.gov_contract)?.to_string(),
                amount: distribute_amount.into(),
            })?,
            funds: vec![],
        }));
    }

    // burn the left amount
    if !left_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.anchor_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Burn {
                amount: left_amount,
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "distribute"),
        ("distribute_amount", &distribute_amount.to_string()),
        ("distributor_payback_amount", &left_amount.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        gov_contract: deps.api.addr_humanize(&state.gov_contract)?.to_string(),
        terraswap_factory: deps
            .api
            .addr_humanize(&state.terraswap_factory)?
            .to_string(),
        anchor_token: deps.api.addr_humanize(&state.anchor_token)?.to_string(),
        reward_factor: state.reward_factor,
    };

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    // migrate the legacy config
    migrate_config(deps.storage)?;
    Ok(Response::default())
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::Api;

    #[test]
    fn proper_migrate() {
        let mut deps = mock_dependencies(&[]);

        // init the contract
        let init_msg = InstantiateMsg {
            gov_contract: "gov".to_string(),
            terraswap_factory: "factory".to_string(),
            anchor_token: "token".to_string(),
            reward_factor: Default::default(),
        };

        let info = mock_info("sender", &[Coin::new(1000000, "uusd")]);
        let res = instantiate(deps.as_mut(), mock_env(), info, init_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // migrate
        let migrate_msg = MigrateMsg {};
        let res = migrate(deps.as_mut(), mock_env(), migrate_msg).unwrap();
        assert_eq!(res, Response::default());

        let config = read_config(&deps.storage).unwrap();
        assert_eq!(
            deps.api.addr_humanize(&config.gov_contract).unwrap(),
            "gov".to_string()
        );
        assert_eq!(
            deps.api.addr_humanize(&config.terraswap_factory).unwrap(),
            "factory".to_string()
        );
        assert_eq!(
            deps.api.addr_humanize(&config.anchor_token).unwrap(),
            "token".to_string()
        );
        assert_eq!(config.reward_factor, Default::default());
    }
}
