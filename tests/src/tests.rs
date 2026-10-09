mod claim_rewards;
mod create_pool;
mod deposit_outcome;
mod initialize;
mod resolve_pool;
mod withdraw_briber_fees;

use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anchor_client::anchor_lang::prelude::clock::SECONDS_PER_DAY;
use reqwest::blocking::Client;
use serde_json::json;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

use crate::utils::{
    fetch_account, fund_account, set_token_balance, setup, OutcomeSetup, TestContext,
    HARDCODED_MARKET, INITIAL_POOL_FUNDING_AMOUNT, INITIAL_STAKE_AMOUNT, PROGRAM_ID,
};

#[test]
fn test_reflex_lifecycle() {
    let yes_ctx = setup(HARDCODED_MARKET);
    initialize::run(&yes_ctx);
    assert_initialized(&yes_ctx);
    run_lifecycle(&yes_ctx, reflex::ResolutionOutcome::Yes);

    let no_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_lifecycle(&no_ctx, reflex::ResolutionOutcome::No);

    let winner_only_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_single_side_lifecycle(&winner_only_ctx, true);

    let loser_only_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_single_side_lifecycle(&loser_only_ctx, false);

    let multi_user_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_multi_user_lifecycle(&multi_user_ctx, true);

    let reverse_claim_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_multi_user_lifecycle(&reverse_claim_ctx, false);

    let incentive_shortfall_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_shortfall_lifecycle(&incentive_shortfall_ctx, true);

    let outcome_shortfall_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_shortfall_lifecycle(&outcome_shortfall_ctx, false);

    let rounding_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_rounding_lifecycle(&rounding_ctx);

    let adversarial_yes_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_adversarial_lifecycle(&adversarial_yes_ctx, reflex::ResolutionOutcome::Yes);

    let adversarial_no_ctx = setup(solana_pubkey::Pubkey::new_unique());
    run_adversarial_lifecycle(&adversarial_no_ctx, reflex::ResolutionOutcome::No);
}

fn run_lifecycle(ctx: &TestContext, outcome: reflex::ResolutionOutcome) {
    let pool = create_pool::run(ctx);
    create_pool::assert_created(ctx, &pool);

    deposit_outcome::assert_wrong_vault_rejected(ctx, &pool);
    deposit_outcome::run(ctx, &pool.yes, INITIAL_STAKE_AMOUNT).expect("initial YES stake failed");

    advance_time(7);

    deposit_outcome::run(ctx, &pool.yes, INITIAL_STAKE_AMOUNT / 2)
        .expect("additional YES stake failed");
    deposit_outcome::run(ctx, &pool.no, INITIAL_STAKE_AMOUNT).expect("initial NO stake failed");
    deposit_outcome::assert_deposited(ctx, &pool);
    claim_rewards::assert_before_resolution_rejected(ctx, &pool);

    advance_time(10);

    resolve_pool::assert_unauthorized_rejected(ctx);
    resolve_pool::run(ctx, outcome);
    resolve_pool::assert_resolved(ctx, outcome);
    resolve_pool::assert_second_resolution_rejected(ctx);

    assert!(
        deposit_outcome::run(ctx, &pool.no, INITIAL_STAKE_AMOUNT / 2).is_err(),
        "deposit must fail after resolution"
    );

    claim_rewards::assert_wrong_signer_rejected(ctx, &pool);
    claim_rewards::assert_wrong_vault_rejected(ctx, &pool);
    claim_rewards::run(ctx, &pool);
    withdraw_briber_fees::run(ctx, &pool);
}

fn run_single_side_lifecycle(ctx: &TestContext, winning_side: bool) {
    let pool = create_pool::run(ctx);
    let outcome = if winning_side { &pool.yes } else { &pool.no };

    deposit_outcome::run(ctx, outcome, INITIAL_STAKE_AMOUNT).expect("single-side stake failed");
    advance_time(10);
    resolve_pool::run(ctx, reflex::ResolutionOutcome::Yes);
    resolve_pool::assert_resolved(ctx, reflex::ResolutionOutcome::Yes);
    claim_rewards::run(ctx, &pool);
}

fn run_multi_user_lifecycle(ctx: &TestContext, second_claims_first: bool) {
    let pool = create_pool::run(ctx);
    let second_staker = Keypair::new();
    fund_account(&second_staker.pubkey());

    let second_yes = OutcomeSetup {
        mint: pool.yes.mint,
        ata: create_pool::fund_outcome(ctx, &second_staker.pubkey(), &pool.yes.mint),
        vault: pool.yes.vault,
    };
    let second_no_ata = create_pool::fund_outcome(ctx, &second_staker.pubkey(), &pool.no.mint);
    let second_position = position_address(&ctx.pdas.pool, &second_staker.pubkey());

    deposit_outcome::run(ctx, &pool.yes, INITIAL_STAKE_AMOUNT)
        .expect("first staker deposit failed");
    advance_time(5);
    deposit_outcome::run_for(
        ctx,
        &second_staker,
        second_position,
        &second_yes,
        INITIAL_STAKE_AMOUNT * 2,
    )
    .expect("second staker deposit failed");
    advance_time(10);

    resolve_pool::run(ctx, reflex::ResolutionOutcome::Yes);
    let claim_first = || {
        claim_rewards::run_for(
            ctx,
            &pool,
            &ctx.accounts.staker,
            ctx.pdas.position,
            pool.yes.ata,
            pool.no.ata,
            second_claims_first,
        )
    };
    let claim_second = || {
        claim_rewards::run_for(
            ctx,
            &pool,
            &second_staker,
            second_position,
            second_yes.ata,
            second_no_ata,
            !second_claims_first,
        )
    };
    let (first_reward, second_reward) = if second_claims_first {
        let second_reward = claim_second();
        let first_reward = claim_first();
        (first_reward, second_reward)
    } else {
        let first_reward = claim_first();
        let second_reward = claim_second();
        (first_reward, second_reward)
    };

    let distributed = first_reward
        .checked_add(second_reward)
        .expect("combined rewards overflow");
    assert!(distributed <= INITIAL_POOL_FUNDING_AMOUNT);
    assert_eq!(
        claim_rewards::token_balance(ctx, &ctx.pdas.pool_vault),
        INITIAL_POOL_FUNDING_AMOUNT - distributed
    );
}

fn run_shortfall_lifecycle(ctx: &TestContext, incentive_shortfall: bool) {
    let pool = create_pool::run(ctx);
    deposit_outcome::run(ctx, &pool.yes, INITIAL_STAKE_AMOUNT)
        .expect("shortfall setup deposit failed");
    advance_time(10);
    resolve_pool::run(ctx, reflex::ResolutionOutcome::Yes);

    if incentive_shortfall {
        set_token_balance(ctx, &ctx.pdas.pool_vault, INITIAL_POOL_FUNDING_AMOUNT - 1);
    } else {
        set_token_balance(
            ctx,
            &pool.yes.vault,
            INITIAL_STAKE_AMOUNT - crate::utils::briber_fee(INITIAL_STAKE_AMOUNT) - 1,
        );
    }

    claim_rewards::assert_shortfall_rejected_atomically(ctx, &pool);
}

fn run_rounding_lifecycle(ctx: &TestContext) {
    let pool = create_pool::run(ctx);
    let second_staker = Keypair::new();
    fund_account(&second_staker.pubkey());
    let second_yes = OutcomeSetup {
        mint: pool.yes.mint,
        ata: create_pool::fund_outcome(ctx, &second_staker.pubkey(), &pool.yes.mint),
        vault: pool.yes.vault,
    };
    let second_no_ata = create_pool::fund_outcome(ctx, &second_staker.pubkey(), &pool.no.mint);
    let second_position = position_address(&ctx.pdas.pool, &second_staker.pubkey());

    deposit_outcome::run(ctx, &pool.yes, 1).expect("tiny first deposit failed");
    deposit_outcome::run_for(ctx, &second_staker, second_position, &second_yes, 2)
        .expect("tiny second deposit failed");
    advance_time(10);
    resolve_pool::run(ctx, reflex::ResolutionOutcome::Yes);

    let first_reward = claim_rewards::run_for(
        ctx,
        &pool,
        &ctx.accounts.staker,
        ctx.pdas.position,
        pool.yes.ata,
        pool.no.ata,
        false,
    );
    let second_reward = claim_rewards::run_for(
        ctx,
        &pool,
        &second_staker,
        second_position,
        second_yes.ata,
        second_no_ata,
        true,
    );
    let distributed = first_reward + second_reward;
    let dust = INITIAL_POOL_FUNDING_AMOUNT - distributed;

    assert!(dust < 2, "rounding dust must be less than claimant count");
    assert_eq!(
        claim_rewards::token_balance(ctx, &ctx.pdas.pool_vault),
        dust
    );
}

fn run_adversarial_lifecycle(ctx: &TestContext, outcome: reflex::ResolutionOutcome) {
    let pool = create_pool::run(ctx);
    let bob = Keypair::new();
    fund_account(&bob.pubkey());
    let bob_yes_ata = create_pool::fund_outcome(ctx, &bob.pubkey(), &pool.yes.mint);
    let bob_no = OutcomeSetup {
        mint: pool.no.mint,
        ata: create_pool::fund_outcome(ctx, &bob.pubkey(), &pool.no.mint),
        vault: pool.no.vault,
    };
    let bob_position = position_address(&ctx.pdas.pool, &bob.pubkey());

    deposit_outcome::run(ctx, &pool.yes, INITIAL_STAKE_AMOUNT).expect("Alice YES deposit failed");
    advance_time(5);
    deposit_outcome::run_for(ctx, &bob, bob_position, &bob_no, INITIAL_STAKE_AMOUNT)
        .expect("Bob NO deposit failed");
    advance_time(10);

    claim_rewards::assert_before_resolution_rejected(ctx, &pool);
    resolve_pool::run(ctx, outcome);
    resolve_pool::assert_resolved(ctx, outcome);
    assert!(
        deposit_outcome::run(ctx, &pool.yes, 1).is_err(),
        "deposit after resolution must fail"
    );

    let claim_alice = |last| {
        claim_rewards::run_for(
            ctx,
            &pool,
            &ctx.accounts.staker,
            ctx.pdas.position,
            pool.yes.ata,
            pool.no.ata,
            last,
        )
    };
    let claim_bob = |last| {
        claim_rewards::run_for(
            ctx,
            &pool,
            &bob,
            bob_position,
            bob_yes_ata,
            bob_no.ata,
            last,
        )
    };

    let (alice_reward, bob_reward) = match outcome {
        reflex::ResolutionOutcome::Yes => {
            let bob_reward = claim_bob(false);
            let alice_reward = claim_alice(true);
            (alice_reward, bob_reward)
        }
        reflex::ResolutionOutcome::No => {
            let alice_reward = claim_alice(false);
            let bob_reward = claim_bob(true);
            (alice_reward, bob_reward)
        }
    };

    match outcome {
        reflex::ResolutionOutcome::Yes => {
            assert_eq!(alice_reward, INITIAL_POOL_FUNDING_AMOUNT);
            assert_eq!(bob_reward, 0);
        }
        reflex::ResolutionOutcome::No => {
            assert_eq!(alice_reward, 0);
            assert_eq!(bob_reward, INITIAL_POOL_FUNDING_AMOUNT);
        }
    }
    assert_eq!(claim_rewards::token_balance(ctx, &ctx.pdas.pool_vault), 0);
}

fn position_address(pool: &Pubkey, staker: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            reflex::constants::POSITION_SEED,
            pool.as_ref(),
            staker.as_ref(),
        ],
        &PROGRAM_ID,
    )
    .0
}

fn assert_initialized(ctx: &TestContext) {
    let config: reflex::Config = fetch_account(ctx, &ctx.pdas.config);

    assert_eq!(config.authority, ctx.accounts.authority.pubkey());
    assert_eq!(config.fee_bps, 0);
    assert_eq!(config.briber_fee_bps, 500);
}

fn advance_time(days: i64) {
    static TEST_TIMESTAMP_MS: AtomicI64 = AtomicI64::new(0);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let _ = TEST_TIMESTAMP_MS.compare_exchange(0, now, Ordering::SeqCst, Ordering::SeqCst);
    let delta = days * SECONDS_PER_DAY as i64 * 1_000;
    let absolute_timestamp = TEST_TIMESTAMP_MS.fetch_add(delta, Ordering::SeqCst) + delta;

    let client = Client::new();
    let rpc_url = "http://127.0.0.1:8899";
    let params = json!({
        "absoluteTimestamp": absolute_timestamp
    });

    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "surfnet_timeTravel",
        "params": [params]
    });

    let res = client
        .post(rpc_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send();

    if res.is_ok() {
        println!("Advance {} days", days)
    } else {
        println!("❌ Fail advancing days")
    }

    // let res = res.unwrap().json::<Value>().unwrap();
}
