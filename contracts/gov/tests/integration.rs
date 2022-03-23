use anchor_token::gov::{ExecuteMsg, InstantiateMsg};
use anchor_token::voting_escrow::InstantiateMsg as VotingEscrowInstantiateMsg;
use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{Addr, Decimal, Uint128};
use terra_multi_test::{AppBuilder, BankKeeper, ContractWrapper, Executor, TerraApp, TerraMock};

const ANCHOR_TOKEN: &str = "anchor";

fn mock_app() -> TerraApp {
    let env = mock_env();
    let api = MockApi::default();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();
    let custom = TerraMock::luna_ust_case();

    AppBuilder::new()
        .with_api(api)
        .with_block(env.block)
        .with_bank(bank)
        .with_storage(storage)
        .with_custom(custom)
        .build()
}

fn store_gov_contract_code(app: &mut TerraApp) -> u64 {
    let gov_contract = Box::new(
        ContractWrapper::new_with_empty(
            anchor_gov::contract::execute,
            anchor_gov::contract::instantiate,
            anchor_gov::contract::query,
        )
        .with_reply_empty(anchor_gov::contract::reply),
    );

    app.store_code(gov_contract)
}

fn store_ve_contract_code(app: &mut TerraApp) -> u64 {
    let ve_contract = Box::new(ContractWrapper::new_with_empty(
        ::anchor_voting_escrow::contract::execute,
        ::anchor_voting_escrow::contract::instantiate,
        ::anchor_voting_escrow::contract::query,
    ));

    app.store_code(ve_contract)
}

#[test]
fn create_gov_and_voting_escrow_contract() {
    let mut router = mock_app();
    let owner = Addr::unchecked("owner");

    let gov_contract_code_id = store_gov_contract_code(&mut router);
    let ve_contract_code_id = store_ve_contract_code(&mut router);

    let msg = InstantiateMsg {
        quorum: Decimal::from_ratio(Uint128::from(1_u64), Uint128::from(10_u64)),
        threshold: Decimal::from_ratio(Uint128::from(1_u64), Uint128::from(2_u64)),
        voting_period: 94097,
        timelock_period: 40327,
        proposal_deposit: Uint128::from(1000000000_u64),
        snapshot_period: 13443,
    };

    let gov = router
        .instantiate_contract(
            gov_contract_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("GOV"),
            None,
        )
        .unwrap();

    let msg = VotingEscrowInstantiateMsg {
        owner: gov.to_string(),
        anchor_token: ANCHOR_TOKEN.to_string(),
        marketing: None,
    };

    let ve = router
        .instantiate_contract(
            ve_contract_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("VOTING_ESCROW"),
            None,
        )
        .unwrap();

    let msg = ExecuteMsg::RegisterContracts {
        anchor_token: ANCHOR_TOKEN.to_string(),
        anchor_voting_escrow: ve.to_string(),
    };

    let _res = router
        .execute_contract(owner.clone(), gov.clone(), &msg, &[])
        .unwrap();

    let res = router
        .execute_contract(owner.clone(), gov.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "Unauthorized");
}
