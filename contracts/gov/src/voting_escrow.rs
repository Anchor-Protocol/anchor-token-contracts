use anchor_token::voting_escrow::{
    ExecuteMsg as VotingEscrowContractExecuteMsg, QueryMsg as VotingEscrowContractQueryMsg,
    VotingPowerResponse,
};
use cosmwasm_std::{
    to_binary, CanonicalAddr, CosmosMsg, Deps, QueryRequest, StdResult, Uint128, WasmMsg, WasmQuery,
};

pub fn query_user_voting_power(
    deps: Deps,
    anchor_voting_escrow: &CanonicalAddr,
    user: &CanonicalAddr,
) -> StdResult<Uint128> {
    let voting_power_res: VotingPowerResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: deps.api.addr_humanize(&anchor_voting_escrow)?.to_string(),
            msg: to_binary(&VotingEscrowContractQueryMsg::UserVotingPower {
                user: deps.api.addr_humanize(user)?.to_string(),
            })?,
        }))?;

    Ok(voting_power_res.voting_power)
}

pub fn query_total_voting_power(
    deps: Deps,
    anchor_voting_escrow: &CanonicalAddr,
) -> StdResult<Uint128> {
    let voting_power_res: VotingPowerResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: deps.api.addr_humanize(&anchor_voting_escrow)?.to_string(),
            msg: to_binary(&VotingEscrowContractQueryMsg::TotalVotingPower {})?,
        }))?;

    Ok(voting_power_res.voting_power)
}

pub fn generate_extend_lock_amount_to_message(
    deps: Deps,
    anchor_voting_escrow: &CanonicalAddr,
    user: &CanonicalAddr,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&anchor_voting_escrow)?.to_string(),
        msg: to_binary(&VotingEscrowContractExecuteMsg::ExtendLockAmountTo {
            user: user.to_string(),
            amount,
        })?,
        funds: vec![],
    }))
}

pub fn generate_extend_lock_time_message(
    deps: Deps,
    anchor_voting_escrow: &CanonicalAddr,
    user: &CanonicalAddr,
    time: u64,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&anchor_voting_escrow)?.to_string(),
        msg: to_binary(&VotingEscrowContractExecuteMsg::ExtendLockTime {
            user: user.to_string(),
            time,
        })?,
        funds: vec![],
    }))
}

pub fn generate_withdraw_message(
    deps: Deps,
    anchor_voting_escrow: &CanonicalAddr,
    user: &CanonicalAddr,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&anchor_voting_escrow)?.to_string(),
        msg: to_binary(&VotingEscrowContractExecuteMsg::Withdraw {
            user: user.to_string(),
            amount,
        })?,
        funds: vec![],
    }))
}
