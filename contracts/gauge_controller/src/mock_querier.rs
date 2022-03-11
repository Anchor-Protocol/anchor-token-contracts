use crate::state::{UserSlopResponse, UserUnlockPeriodResponse, VotingEscrowContractQueryMsg};
use crate::utils::{get_period, WEEK};

use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Coin, ContractResult, Decimal, Empty, OwnedDeps, Querier,
    QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};

pub(crate) const BASE_TIME: u64 = WEEK * 1000 + 10;

pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        api: MockApi::default(),
        storage: MockStorage::default(),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: _,
                msg,
            }) => match from_binary(msg).unwrap() {
                VotingEscrowContractQueryMsg::LastUserSlope { user } => {
                    let slope = if user == String::from("user_1") {
                        Decimal::from_ratio(Uint128::from(998244353_u64), Uint128::from(100_u64))
                    } else if user == String::from("user_2") {
                        Decimal::from_ratio(Uint128::from(1000000007_u64), Uint128::from(66_u64))
                    } else {
                        panic!("INVALID USER");
                    };
                    SystemResult::Ok(ContractResult::Ok(
                        to_binary(&UserSlopResponse { slope: slope }).unwrap(),
                    ))
                }
                VotingEscrowContractQueryMsg::UserUnlockPeriod { user } => {
                    let time = if user == String::from("user_1") {
                        BASE_TIME + WEEK * 100
                    } else if user == String::from("user_2") {
                        BASE_TIME + WEEK * 66
                    } else {
                        panic!("INVALID USER");
                    };
                    SystemResult::Ok(ContractResult::Ok(
                        to_binary(&UserUnlockPeriodResponse {
                            unlock_period: get_period(time),
                        })
                        .unwrap(),
                    ))
                }
            },
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier { base: base }
    }
}
