use anchor_token::voting_escrow::{QueryMsg as VotingEscrowContractQueyMsg, VotingPowerResponse};
use cosmwasm_std::{to_binary, CanonicalAddr, Deps, QueryRequest, StdResult, Uint128, WasmQuery};

pub fn query_user_voting_power(
    deps: Deps,
    anchor_voting_escrow: CanonicalAddr,
    user: &CanonicalAddr,
) -> StdResult<Uint128> {
    let voting_power_res: VotingPowerResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: deps.api.addr_humanize(&anchor_voting_escrow)?.to_string(),
            msg: to_binary(&VotingEscrowContractQueyMsg::UserVotingPower {
                user: deps.api.addr_humanize(user)?.to_string(),
            })?,
        }))?;

    Ok(voting_power_res.voting_power)
}
