use anchor_token::gov::{
    Cw20HookMsg, ExecuteMsg as GovExecuteMsg, InstantiateMsg as GovInstantiateMsg, PollResponse,
    QueryMsg as GovQueryMsg, VoteOption,
};
use anchor_token::voting_escrow::{
    ExecuteMsg as VotingEscrowExecuteMsg, InstantiateMsg as VotingEscrowInstantiateMsg,
    QueryMsg as VotingEscrowQueryMsg, VotingPowerResponse,
};
use anyhow::Result;
use astroport::token::InstantiateMsg as TokenInstantiateMsg;
use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{to_binary, Addr, Decimal, Uint128};
use cw20::{Cw20ExecuteMsg, MinterResponse};
use terra_multi_test::{
    AppBuilder, AppResponse, BankKeeper, ContractWrapper, Executor, TerraApp, TerraMock,
};

const OWNER: &str = "owner";
const ALICE: &str = "alice";
const BOB: &str = "bob";
const COLLECTOR: &str = "collector";

const DAY: u64 = 86400;
const WEEK: u64 = 7 * DAY;
const YEAR: u64 = 365 * DAY;
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
        decimals: 6,
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
        voting_period: 188194,
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

    router
        .execute_contract(owner.clone(), gov.clone(), &msg, &[])
        .unwrap();

    return (router, anchor_token, gov, ve);
}

fn mint_token(router: &mut TerraApp, token: &Addr, recipient: &Addr, amount: Uint128) {
    let amount = amount * Uint128::from(1000000_u64);
    let msg = Cw20ExecuteMsg::Mint {
        recipient: recipient.to_string(),
        amount,
    };
    router
        .execute_contract(Addr::unchecked(OWNER), token.clone(), &msg, &[])
        .unwrap();
}

fn extend_lock_time(
    router: &mut TerraApp,
    gov: &Addr,
    user: &Addr,
    time: u64,
) -> Result<AppResponse> {
    let msg = GovExecuteMsg::ExtendLockTime { time };
    router.execute_contract(user.clone(), gov.clone(), &msg, &[])
}

fn extend_lock_amount(
    router: &mut TerraApp,
    anchor_token: &Addr,
    gov: &Addr,
    user: &Addr,
    amount: Uint128,
) -> Result<AppResponse> {
    let amount = amount * Uint128::from(1000000_u64);
    let msg = Cw20ExecuteMsg::Send {
        contract: gov.to_string(),
        amount: Uint128::from(amount),
        msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
    };
    router.execute_contract(user.clone(), anchor_token.clone(), &msg, &[])
}

fn withdraw_voting_tokens(
    router: &mut TerraApp,
    gov: &Addr,
    user: &Addr,
    amount: Uint128,
) -> Result<AppResponse> {
    let amount = amount * Uint128::from(1000000_u64);
    let msg = GovExecuteMsg::WithdrawVotingTokens {
        amount: Some(amount),
    };
    router.execute_contract(user.clone(), gov.clone(), &msg, &[])
}

fn collect_rewards(router: &mut TerraApp, anchor_token: &Addr, gov: &Addr, amount: Uint128) {
    mint_token(router, &anchor_token, &Addr::unchecked(COLLECTOR), amount);
    let amount = amount * Uint128::from(1000000_u64);
    let msg = Cw20ExecuteMsg::Transfer {
        recipient: gov.to_string(),
        amount,
    };
    router
        .execute_contract(Addr::unchecked(COLLECTOR), anchor_token.clone(), &msg, &[])
        .unwrap();
}

fn create_poll(
    router: &mut TerraApp,
    anchor_token: &Addr,
    gov: &Addr,
    user: &Addr,
    amount: Uint128,
) -> Result<AppResponse> {
    mint_token(router, &anchor_token, &user, amount);
    let amount = amount * Uint128::from(1000000_u64);
    let msg = Cw20ExecuteMsg::Send {
        contract: gov.to_string(),
        amount: Uint128::from(amount),
        msg: to_binary(&Cw20HookMsg::CreatePoll {
            title: "poll test".to_string(),
            description: "poll description".to_string(),
            link: None,
            execute_msgs: None,
        })
        .unwrap(),
    };
    router.execute_contract(user.clone(), anchor_token.clone(), &msg, &[])
}

fn create_poll_with_id(
    router: &mut TerraApp,
    anchor_token: &Addr,
    gov: &Addr,
    user: &Addr,
    amount: Uint128,
) -> u64 {
    let events = create_poll(router, &anchor_token, &gov, &user, amount)
        .unwrap()
        .events;
    for event in events {
        for attr in event.attributes {
            if attr.key == "poll_id" {
                return attr.value.parse().unwrap();
            }
        }
    }
    panic!("create_poll failed");
}

fn cast_vote(
    router: &mut TerraApp,
    gov: &Addr,
    user: &Addr,
    poll_id: u64,
    vote: VoteOption,
    amount: Uint128,
) -> Result<AppResponse> {
    let amount = amount * Uint128::from(1000000_u64);
    let msg = GovExecuteMsg::CastVote {
        poll_id,
        vote,
        amount,
    };
    router.execute_contract(user.clone(), gov.clone(), &msg, &[])
}

fn snapshot_poll(
    router: &mut TerraApp,
    gov: &Addr,
    user: &Addr,
    poll_id: u64,
) -> Result<AppResponse> {
    let msg = GovExecuteMsg::SnapshotPoll { poll_id };
    router.execute_contract(user.clone(), gov.clone(), &msg, &[])
}

fn end_poll(router: &mut TerraApp, gov: &Addr, user: &Addr, poll_id: u64) -> Result<AppResponse> {
    let msg = GovExecuteMsg::EndPoll { poll_id };
    router.execute_contract(user.clone(), gov.clone(), &msg, &[])
}

fn query_voting_power(router: &TerraApp, ve: &Addr, user: &Addr) -> Uint128 {
    let res: VotingPowerResponse = router
        .wrap()
        .query_wasm_smart(
            ve.clone(),
            &VotingEscrowQueryMsg::UserVotingPower {
                user: user.to_string(),
            },
        )
        .unwrap();
    (res.voting_power + Uint128::from(500000_u64)) / Uint128::from(1000000_u64)
}

fn query_poll(router: &TerraApp, gov: &Addr, poll_id: u64) -> PollResponse {
    let res: PollResponse = router
        .wrap()
        .query_wasm_smart(gov.clone(), &GovQueryMsg::Poll { poll_id })
        .unwrap();
    res
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
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, _ve) = create_contracts();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(100_u64));

    let res = extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(100_u64),
    )
    .unwrap_err();

    assert_eq!(res.to_string(), "Lock does not exist");
}

#[test]
fn test_invalid_unlocking_time() {
    let alice = Addr::unchecked(ALICE);
    let (mut router, _anchor_token, gov, _ve) = create_contracts();

    let res = extend_lock_time(&mut router, &gov, &alice, YEAR / 2).unwrap_err();
    assert_eq!(res.to_string(), "Lock time must be within the limits");

    let res = extend_lock_time(&mut router, &gov, &alice, YEAR * 6).unwrap_err();
    assert_eq!(res.to_string(), "Lock time must be within the limits");

    extend_lock_time(&mut router, &gov, &alice, YEAR * 3).unwrap();

    let res = extend_lock_time(&mut router, &gov, &alice, YEAR + WEEK).unwrap_err();
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
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR * 2).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(200_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(126_u64)
    );

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365 / 2;
        b.time = b.time.plus_seconds(YEAR / 2);
    });

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(95_u64)
    );

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(190_u64)
    );

    extend_lock_time(&mut router, &gov, &alice, YEAR).unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(315_u64)
    );
}

#[test]
fn test_lock_token_and_withdraw_and_lock_again() {
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(200_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(200_u64),
    )
    .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365 / 2;
        b.time = b.time.plus_seconds(YEAR / 2);
    });

    let res =
        withdraw_voting_tokens(&mut router, &gov, &alice, Uint128::from(100_u64)).unwrap_err();
    assert_eq!(res.to_string(), "The lock time has not yet expired");

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(65_u64)
    );

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365 / 2;
        b.time = b.time.plus_seconds(YEAR / 2);
    });

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(0_u64)
    );

    withdraw_voting_tokens(&mut router, &gov, &alice, Uint128::from(100_u64)).unwrap();

    extend_lock_time(&mut router, &gov, &alice, YEAR * 4).unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(250_u64)
    );
}

#[test]
fn test_lock_token_and_withdraw_multiple_times() {
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, _ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(100_u64),
    )
    .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365;
        b.time = b.time.plus_seconds(YEAR);
    });

    withdraw_voting_tokens(&mut router, &gov, &alice, Uint128::from(20_u64)).unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 7;
        b.time = b.time.plus_seconds(WEEK);
    });

    withdraw_voting_tokens(&mut router, &gov, &alice, Uint128::from(70_u64)).unwrap();

    let res = withdraw_voting_tokens(&mut router, &gov, &alice, Uint128::from(11_u64)).unwrap_err();

    assert_eq!(
        res.to_string(),
        "User is trying to withdraw too many tokens"
    );

    withdraw_voting_tokens(&mut router, &gov, &alice, Uint128::from(10_u64)).unwrap();
}

#[test]
fn test_lock_token_and_get_rewards() {
    let alice = Addr::unchecked(ALICE);
    let bob = Addr::unchecked(BOB);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(100_u64),
    )
    .unwrap();

    collect_rewards(&mut router, &anchor_token, &gov, Uint128::from(50_u64));

    extend_lock_time(&mut router, &gov, &bob, YEAR).unwrap();

    mint_token(&mut router, &anchor_token, &bob, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &bob,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(64_u64)
    );

    assert_eq!(
        query_voting_power(&router, &ve, &bob),
        Uint128::from(42_u64)
    );

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 365;
        b.time = b.time.plus_seconds(YEAR);
    });

    collect_rewards(&mut router, &anchor_token, &gov, Uint128::from(50_u64));

    withdraw_voting_tokens(&mut router, &gov, &alice, Uint128::from(180_u64)).unwrap();
    withdraw_voting_tokens(&mut router, &gov, &alice, Uint128::from(181_u64)).unwrap_err();
    withdraw_voting_tokens(&mut router, &gov, &bob, Uint128::from(119_u64)).unwrap();
    withdraw_voting_tokens(&mut router, &gov, &bob, Uint128::from(120_u64)).unwrap_err();
}

#[test]
fn test_create_poll_token_requirement() {
    let alice = Addr::unchecked(ALICE);
    let (mut router, anchor_token, gov, _ve) = create_contracts();

    let res = create_poll(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(999_u64),
    )
    .unwrap_err();

    assert_eq!(res.to_string(), "Must deposit more than 1000000000 token");

    create_poll(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    )
    .unwrap();

    create_poll(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(10000_u64),
    )
    .unwrap();
}

#[test]
fn test_cast_vote_with_different_amount_of_veanc() {
    let alice = Addr::unchecked(ALICE);
    let bob = Addr::unchecked(BOB);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    let poll_id = create_poll_with_id(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    );

    let res = cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(0_u64),
    )
    .unwrap_err();

    assert_eq!(res.to_string(), "User is trying to cast zero vote");

    let res = cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(100_u64),
    )
    .unwrap_err();

    assert_eq!(res.to_string(), "User does not have enough staked tokens");

    extend_lock_time(&mut router, &gov, &bob, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &bob, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &bob,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &bob),
        Uint128::from(250_u64)
    );

    let res = cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(251_u64),
    )
    .unwrap_err();

    assert_eq!(res.to_string(), "User does not have enough staked tokens");

    cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(250_u64),
    )
    .unwrap();
}

#[test]
fn test_votes_counting() {
    let alice = Addr::unchecked(ALICE);
    let bob = Addr::unchecked(BOB);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(1000_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    )
    .unwrap();

    let poll_id = create_poll_with_id(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    );

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(2500_u64)
    );

    extend_lock_time(&mut router, &gov, &bob, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &bob, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &bob,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &bob),
        Uint128::from(250_u64)
    );

    cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(249_u64),
    )
    .unwrap();

    cast_vote(
        &mut router,
        &gov,
        &alice,
        poll_id,
        VoteOption::No,
        Uint128::from(251_u64),
    )
    .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 28;
        b.time = b.time.plus_seconds(WEEK * 4);
    });

    end_poll(&mut router, &gov, &alice, poll_id).unwrap();

    let res = query_poll(&router, &gov, poll_id);

    assert_eq!(res.yes_votes, Uint128::from(249_000000_u64));
    assert_eq!(res.no_votes, Uint128::from(251_000000_u64));
}

#[test]
fn test_total_voting_power_counting() {
    let alice = Addr::unchecked(ALICE);
    let bob = Addr::unchecked(BOB);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(1000_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    )
    .unwrap();

    let poll_id = create_poll_with_id(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    );

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(2500_u64)
    );

    extend_lock_time(&mut router, &gov, &bob, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &bob, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &bob,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &bob),
        Uint128::from(250_u64)
    );

    cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(249_u64),
    )
    .unwrap();

    cast_vote(
        &mut router,
        &gov,
        &alice,
        poll_id,
        VoteOption::No,
        Uint128::from(251_u64),
    )
    .unwrap();

    router.update_block(|b| {
        b.height += 180000;
        b.time = b.time.plus_seconds(WEEK * 4);
    });

    snapshot_poll(&mut router, &gov, &alice, poll_id).unwrap();

    let res = query_poll(&router, &gov, poll_id);

    assert_eq!(res.staked_amount, Some(Uint128::from(2697_115385_u64)));
}

#[test]
fn test_rejected_poll_quorum_not_reached() {
    let alice = Addr::unchecked(ALICE);
    let bob = Addr::unchecked(BOB);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(1000_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    )
    .unwrap();

    let poll_id = create_poll_with_id(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    );

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(2500_u64)
    );

    extend_lock_time(&mut router, &gov, &bob, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &bob, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &bob,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &bob),
        Uint128::from(250_u64)
    );

    cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(24_u64),
    )
    .unwrap();

    cast_vote(
        &mut router,
        &gov,
        &alice,
        poll_id,
        VoteOption::Yes,
        Uint128::from(240_u64),
    )
    .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 28;
        b.time = b.time.plus_seconds(WEEK * 4);
    });

    let events = end_poll(&mut router, &gov, &alice, poll_id).unwrap().events;

    for event in events {
        for attr in event.attributes {
            if attr.key == "poll_id" {
                assert_eq!(attr.value, "1");
            } else if attr.key == "passed" {
                assert_eq!(attr.value, "false");
            } else if attr.key == "rejected_reason" {
                assert_eq!(attr.value, "Quorum not reached");
            }
        }
    }
}

#[test]
fn test_rejected_poll_threshold_not_reached() {
    let alice = Addr::unchecked(ALICE);
    let bob = Addr::unchecked(BOB);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(1000_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    )
    .unwrap();

    let poll_id = create_poll_with_id(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    );

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(2500_u64)
    );

    extend_lock_time(&mut router, &gov, &bob, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &bob, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &bob,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &bob),
        Uint128::from(250_u64)
    );

    cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(25_u64),
    )
    .unwrap();

    cast_vote(
        &mut router,
        &gov,
        &alice,
        poll_id,
        VoteOption::No,
        Uint128::from(250_u64),
    )
    .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 28;
        b.time = b.time.plus_seconds(WEEK * 4);
    });

    let events = end_poll(&mut router, &gov, &alice, poll_id).unwrap().events;

    for event in events {
        for attr in event.attributes {
            if attr.key == "poll_id" {
                assert_eq!(attr.value, "1");
            } else if attr.key == "passed" {
                assert_eq!(attr.value, "false");
            } else if attr.key == "rejected_reason" {
                assert_eq!(attr.value, "Threshold not reached");
            }
        }
    }
}

#[test]
fn test_passed_poll() {
    let alice = Addr::unchecked(ALICE);
    let bob = Addr::unchecked(BOB);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(1000_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    )
    .unwrap();

    let poll_id = create_poll_with_id(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    );

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(2500_u64)
    );

    extend_lock_time(&mut router, &gov, &bob, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &bob, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &bob,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &bob),
        Uint128::from(250_u64)
    );

    cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(25_u64),
    )
    .unwrap();

    cast_vote(
        &mut router,
        &gov,
        &alice,
        poll_id,
        VoteOption::Yes,
        Uint128::from(250_u64),
    )
    .unwrap();

    router.update_block(|b| {
        b.height += BLOCKS_PER_DAY * 28;
        b.time = b.time.plus_seconds(WEEK * 4);
    });

    let events = end_poll(&mut router, &gov, &alice, poll_id).unwrap().events;

    for event in events {
        for attr in event.attributes {
            if attr.key == "poll_id" {
                assert_eq!(attr.value, "1");
            } else if attr.key == "passed" {
                assert_eq!(attr.value, "true");
            } else if attr.key == "rejected_reason" {
                assert_eq!(attr.value, "None");
            }
        }
    }
}

#[test]
fn test_passed_poll_total_votes_greater_than_total_staked() {
    let alice = Addr::unchecked(ALICE);
    let bob = Addr::unchecked(BOB);
    let (mut router, anchor_token, gov, ve) = create_contracts();

    extend_lock_time(&mut router, &gov, &alice, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &alice, Uint128::from(1000_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    )
    .unwrap();

    let poll_id = create_poll_with_id(
        &mut router,
        &anchor_token,
        &gov,
        &alice,
        Uint128::from(1000_u64),
    );

    assert_eq!(
        query_voting_power(&router, &ve, &alice),
        Uint128::from(2500_u64)
    );

    extend_lock_time(&mut router, &gov, &bob, YEAR * 4 - WEEK).unwrap();

    mint_token(&mut router, &anchor_token, &bob, Uint128::from(100_u64));

    extend_lock_amount(
        &mut router,
        &anchor_token,
        &gov,
        &bob,
        Uint128::from(100_u64),
    )
    .unwrap();

    assert_eq!(
        query_voting_power(&router, &ve, &bob),
        Uint128::from(250_u64)
    );

    cast_vote(
        &mut router,
        &gov,
        &bob,
        poll_id,
        VoteOption::Yes,
        Uint128::from(250_u64),
    )
    .unwrap();

    cast_vote(
        &mut router,
        &gov,
        &alice,
        poll_id,
        VoteOption::Yes,
        Uint128::from(2500_u64),
    )
    .unwrap();

    router.update_block(|b| {
        b.height += 180000;
        b.time = b.time.plus_seconds(WEEK * 4);
    });

    snapshot_poll(&mut router, &gov, &alice, poll_id).unwrap();

    let res = query_poll(&router, &gov, poll_id);

    assert_eq!(res.yes_votes, Uint128::from(2750_000000_u64));
    assert_eq!(res.staked_amount, Some(Uint128::from(2697_115385_u64)));
}
