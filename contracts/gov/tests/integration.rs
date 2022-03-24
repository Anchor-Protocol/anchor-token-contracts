use anchor_token::gov::{
    Cw20HookMsg, ExecuteMsg as GovExecuteMsg, InstantiateMsg as GovInstantiateMsg,
};
use anchor_token::voting_escrow::{
    ExecuteMsg as VotingEscrowExecuteMsg, InstantiateMsg as VotingEscrowInstantiateMsg,
    QueryMsg as VotingEscrowQueryMsg, VotingPowerResponse,
};
use astroport::token::InstantiateMsg as TokenInstantiateMsg;
use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{to_binary, Addr, Decimal, Uint128};
use cw20::{Cw20ExecuteMsg, MinterResponse};
use terra_multi_test::{AppBuilder, BankKeeper, ContractWrapper, Executor, TerraApp, TerraMock};

const OWNER: &str = "owner";
const ALICE: &str = "alice";

const WEEK: u64 = 7 * 86400;
const YEAR: u64 = 365 * 86400;
const BLOCKS_PER_DAY: u64 = 17280;

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
        anchor_voting_escrow::contract::execute,
        anchor_voting_escrow::contract::instantiate,
        anchor_voting_escrow::contract::query,
    ));

    app.store_code(ve_contract)
}

fn store_token_contract_code(app: &mut TerraApp) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ));

    app.store_code(token_contract)
}

fn create_contracts() -> (TerraApp, Addr, Addr, Addr) {
    let mut router = mock_app();
    let owner = Addr::unchecked(OWNER);

    let gov_contract_code_id = store_gov_contract_code(&mut router);
    let ve_contract_code_id = store_ve_contract_code(&mut router);
    let token_contract_code_id = store_token_contract_code(&mut router);

    let msg = TokenInstantiateMsg {
        name: "anchor_token".to_string(),
        symbol: "ANC".to_string(),
        decimals: 5,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: String::from(owner.clone()),
            cap: None,
        }),
    };

    let anchor_token = router
        .instantiate_contract(
            token_contract_code_id,
            owner.clone(),
            &msg,
            &[],
            "anchor_token",
            None,
        )
        .unwrap();

    let msg = GovInstantiateMsg {
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
        anchor_token: anchor_token.to_string(),
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

    let msg = GovExecuteMsg::RegisterContracts {
        anchor_token: anchor_token.to_string(),
        anchor_voting_escrow: ve.to_string(),
    };

    let _res = router
        .execute_contract(owner.clone(), gov.clone(), &msg, &[])
        .unwrap();

    return (router, anchor_token, gov, ve);
}

fn mint_token(
    router: &mut TerraApp,
    token: &Addr,
    owner: &Addr,
    recipient: &Addr,
    amount: Uint128,
) {
    let msg = Cw20ExecuteMsg::Mint {
        recipient: recipient.to_string(),
        amount,
    };
    router
        .execute_contract(owner.clone(), token.clone(), &msg, &[])
        .unwrap();
}

#[test]
fn test_register_contracts_twice() {
    let owner = Addr::unchecked(OWNER);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    let msg = GovExecuteMsg::RegisterContracts {
        anchor_token: anchor_token.to_string(),
        anchor_voting_escrow: ve.to_string(),
    };

    let res = router
        .execute_contract(owner.clone(), gov.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "Unauthorized");
}

#[test]
fn test_deposit_without_setting_lock_time() {
    let owner = Addr::unchecked(OWNER);
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, _ve) = create_contracts();

    mint_token(
        &mut router,
        &anchor_token,
        &owner,
        &alice,
        Uint128::from(100_u64),
    );

    let msg = Cw20ExecuteMsg::Send {
        contract: gov.to_string(),
        amount: Uint128::from(100_u64),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    };

    let res = router
        .execute_contract(alice.clone(), anchor_token.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "Lock does not exist");
}

#[test]
fn test_invalid_unlocking_time() {
    let alice = Addr::unchecked(ALICE);
    let (mut router, _anchor_token, gov, _ve) = create_contracts();

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR / 2 };

    let res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "Lock time must be within the limits");

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR * 6 };

    let res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "Lock time must be within the limits");

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR * 3 };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR + WEEK };

    let res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "Lock time must be within the limits");
}

#[test]
fn check_permission_of_ve_contract() {
    let alice = Addr::unchecked(ALICE);
    let (mut router, _anchor_token, _gov, ve) = create_contracts();

    let msg = VotingEscrowExecuteMsg::ExtendLockTime {
        user: alice.to_string(),
        time: YEAR * 2,
    };

    let res = router
        .execute_contract(alice.clone(), ve.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "Unauthorized");

    let msg = VotingEscrowExecuteMsg::ExtendLockAmount {
        user: alice.to_string(),
        amount: Uint128::from(100_u64),
    };

    let res = router
        .execute_contract(alice.clone(), ve.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "Unauthorized");
}

#[test]
fn test_set_unlocking_time_and_stake_several_times() {
    let owner = Addr::unchecked(OWNER);
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR * 2 };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    mint_token(
        &mut router,
        &anchor_token,
        &owner,
        &alice,
        Uint128::from(200_u64),
    );

    let msg = Cw20ExecuteMsg::Send {
        contract: gov.to_string(),
        amount: Uint128::from(100_u64),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    };

    let _res = router
        .execute_contract(alice.clone(), anchor_token.clone(), &msg, &[])
        .unwrap();

    let res: VotingPowerResponse = router
        .wrap()
        .query_wasm_smart(
            ve.clone(),
            &VotingEscrowQueryMsg::UserVotingPower {
                user: alice.to_string(),
            },
        )
        .unwrap();

    assert_eq!(res.voting_power, Uint128::from(126_u64));

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365 / 2;
        b.time = b.time.plus_seconds(YEAR / 2);
    });

    let res: VotingPowerResponse = router
        .wrap()
        .query_wasm_smart(
            ve.clone(),
            &VotingEscrowQueryMsg::UserVotingPower {
                user: alice.to_string(),
            },
        )
        .unwrap();

    assert_eq!(res.voting_power, Uint128::from(95_u64));

    let msg = Cw20ExecuteMsg::Send {
        contract: gov.to_string(),
        amount: Uint128::from(100_u64),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    };

    let _res = router
        .execute_contract(alice.clone(), anchor_token.clone(), &msg, &[])
        .unwrap();

    let res: VotingPowerResponse = router
        .wrap()
        .query_wasm_smart(
            ve.clone(),
            &VotingEscrowQueryMsg::UserVotingPower {
                user: alice.to_string(),
            },
        )
        .unwrap();

    assert_eq!(res.voting_power, Uint128::from(189_u64));

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    let res: VotingPowerResponse = router
        .wrap()
        .query_wasm_smart(
            ve.clone(),
            &VotingEscrowQueryMsg::UserVotingPower {
                user: alice.to_string(),
            },
        )
        .unwrap();

    assert_eq!(res.voting_power, Uint128::from(314_u64));
}

#[test]
fn test_lock_token_and_withdraw_and_lock_again() {
    let owner = Addr::unchecked(OWNER);
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    mint_token(
        &mut router,
        &anchor_token,
        &owner,
        &alice,
        Uint128::from(200_u64),
    );

    let msg = Cw20ExecuteMsg::Send {
        contract: gov.to_string(),
        amount: Uint128::from(200_u64),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    };

    let _res = router
        .execute_contract(alice.clone(), anchor_token.clone(), &msg, &[])
        .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365 / 2;
        b.time = b.time.plus_seconds(YEAR / 2);
    });

    let msg = GovExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(100_u64)),
    };

    let res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(res.to_string(), "The lock time has not yet expired");

    let res: VotingPowerResponse = router
        .wrap()
        .query_wasm_smart(
            ve.clone(),
            &VotingEscrowQueryMsg::UserVotingPower {
                user: alice.to_string(),
            },
        )
        .unwrap();

    assert_eq!(res.voting_power, Uint128::from(65_u64));

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365 / 2;
        b.time = b.time.plus_seconds(YEAR / 2);
    });

    let res: VotingPowerResponse = router
        .wrap()
        .query_wasm_smart(
            ve.clone(),
            &VotingEscrowQueryMsg::UserVotingPower {
                user: alice.to_string(),
            },
        )
        .unwrap();

    assert_eq!(res.voting_power, Uint128::from(0_u64));

    let msg = GovExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(100_u64)),
    };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR * 4 };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    let res: VotingPowerResponse = router
        .wrap()
        .query_wasm_smart(
            ve.clone(),
            &VotingEscrowQueryMsg::UserVotingPower {
                user: alice.to_string(),
            },
        )
        .unwrap();

    assert_eq!(res.voting_power, Uint128::from(250_u64));
}

#[test]
fn test_lock_token_and_withdraw_multiple_times() {
    let owner = Addr::unchecked(OWNER);
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, _ve) = create_contracts();

    let msg = GovExecuteMsg::ExtendLockTime { time: YEAR };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    mint_token(
        &mut router,
        &anchor_token,
        &owner,
        &alice,
        Uint128::from(100_u64),
    );

    let msg = Cw20ExecuteMsg::Send {
        contract: gov.to_string(),
        amount: Uint128::from(100_u64),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    };

    let _res = router
        .execute_contract(alice.clone(), anchor_token.clone(), &msg, &[])
        .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365;
        b.time = b.time.plus_seconds(YEAR);
    });

    let msg = GovExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(20_u64)),
    };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 7;
        b.time = b.time.plus_seconds(WEEK);
    });

    let msg = GovExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(70_u64)),
    };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();

    let msg = GovExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11_u64)),
    };

    let res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(
        res.to_string(),
        "User is trying to withdraw too many tokens"
    );

    let msg = GovExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(10_u64)),
    };

    let _res = router
        .execute_contract(alice.clone(), gov.clone(), &msg, &[])
        .unwrap();
}
