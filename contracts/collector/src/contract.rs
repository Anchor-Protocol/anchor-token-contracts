use cosmwasm_std::{
    log, to_binary, Api, Binary, Coin, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, InitResponse, MigrateResponse, MigrateResult, Querier, StdError, StdResult,
    Storage, WasmMsg,
};

use crate::state::{read_config, store_config, Config};

use anchor_token::collector::{ConfigResponse, HandleMsg, InitMsg, MigrateMsg, QueryMsg};
use cw20::Cw20HandleMsg;
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::HandleMsg as TerraswapHandleMsg;
use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            gov_contract: deps.api.canonical_address(&msg.gov_contract)?,
            terraswap_factory: deps.api.canonical_address(&msg.terraswap_factory)?,
            anchor_token: deps.api.canonical_address(&msg.anchor_token)?,
            distributor_contract: deps.api.canonical_address(&msg.distributor_contract)?,
            reward_factor: msg.reward_factor,
        },
    )?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::UpdateConfig { reward_factor } => update_config(deps, env, reward_factor),
        HandleMsg::Sweep { denom } => sweep(deps, env, denom),
        HandleMsg::Distribute {} => distribute(deps, env),
    }
}
pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    reward_factor: Option<Decimal>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if deps.api.canonical_address(&env.message.sender)? != config.gov_contract {
        return Err(StdError::unauthorized());
    }

    if let Some(reward_factor) = reward_factor {
        config.reward_factor = reward_factor;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse::default())
}
/// Sweep
/// Anyone can execute sweep function to swap
/// asset token => ANC token and distribute
/// result ANC token to gov contract
pub fn sweep<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    denom: String,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let anchor_token = deps.api.human_address(&config.anchor_token)?;
    let terraswap_factory_raw = deps.api.human_address(&config.terraswap_factory)?;

    let pair_info: PairInfo = query_pair_info(
        &deps,
        &terraswap_factory_raw,
        &[
            AssetInfo::NativeToken {
                denom: denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: anchor_token.clone(),
            },
        ],
    )?;

    let amount = query_balance(&deps, &env.contract.address, denom.to_string())?;
    let swap_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: denom.to_string(),
        },
        amount,
    };

    // deduct tax first
    let amount = (swap_asset.deduct_tax(&deps)?).amount;
    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_info.contract_addr,
                msg: to_binary(&TerraswapHandleMsg::Swap {
                    offer_asset: Asset {
                        amount,
                        ..swap_asset
                    },
                    max_spread: None,
                    belief_price: None,
                    to: None,
                })?,
                send: vec![Coin {
                    denom: denom.to_string(),
                    amount,
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address,
                msg: to_binary(&HandleMsg::Distribute {})?,
                send: vec![],
            }),
        ],
        log: vec![
            log("action", "sweep"),
            log(
                "collected_rewards",
                format!("{:?}{:?}", amount.to_string(), denom),
            ),
        ],
        data: None,
    })
}

// Only contract itself can execute distribute function
pub fn distribute<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    if env.message.sender != env.contract.address {
        return Err(StdError::unauthorized());
    }

    let config: Config = read_config(&deps.storage)?;
    let amount = query_token_balance(
        &deps,
        &deps.api.human_address(&config.anchor_token)?,
        &env.contract.address,
    )?;

    let distribute_amount = amount * config.reward_factor;
    let left_amount = (amount - distribute_amount)?;

    let mut messages: Vec<CosmosMsg> = vec![];

    if !distribute_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.anchor_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: deps.api.human_address(&config.gov_contract)?,
                amount: distribute_amount,
            })?,
            send: vec![],
        }));
    }

    if !left_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.anchor_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: deps.api.human_address(&config.distributor_contract)?,
                amount: left_amount,
            })?,
            send: vec![],
        }));
    }

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "distribute"),
            log("distribute_amount", distribute_amount.to_string()),
            log("distributor_payback_amount", left_amount.to_string()),
        ],
        data: None,
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        gov_contract: deps.api.human_address(&state.gov_contract)?,
        terraswap_factory: deps.api.human_address(&state.terraswap_factory)?,
        anchor_token: deps.api.human_address(&state.anchor_token)?,
        distributor_contract: deps.api.human_address(&state.distributor_contract)?,
        reward_factor: state.reward_factor,
    };

    Ok(resp)
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
