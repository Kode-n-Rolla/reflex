use std::rc::Rc;

use solana_signer::Signer;

use anchor_client::anchor_lang::prelude::program_pack::Pack;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;

use crate::utils::{
    briber_fee, fetch_account, OutcomeSetup, PoolSetup, TestContext, INITIAL_OUTCOME_AMOUNT,
    INITIAL_STAKE_AMOUNT,
};

pub(super) fn run(ctx: &TestContext, outcome: &OutcomeSetup, amount: u64) -> Result<(), String> {
    run_for(
        ctx,
        &ctx.accounts.staker,
        ctx.pdas.position,
        outcome,
        amount,
    )
}

pub(super) fn run_for(
    ctx: &TestContext,
    staker: &Keypair,
    position: Pubkey,
    outcome: &OutcomeSetup,
    amount: u64,
) -> Result<(), String> {
    let signature = ctx
        .program
        .request()
        .accounts(reflex::accounts::DepositOutcome {
            staker: staker.pubkey(),
            pool: ctx.pdas.pool,
            market: ctx.market,
            position,
            outcome_mint: outcome.mint,
            pool_outcome_vault: outcome.vault,
            staker_ata: outcome.ata,
            token_program: spl_token_interface::ID,
            system_program: solana_sdk_ids::system_program::id(),
        })
        .args(reflex::instruction::DepositOutcome { amount })
        .payer(Rc::from(staker.insecure_clone()))
        .send()
        .map_err(|error| error.to_string())?;

    println!("✅ Stake outcome transaction: {signature}");
    Ok(())
}

pub(super) fn assert_deposited(ctx: &TestContext, setup: &PoolSetup) {
    let pool: reflex::Pool = fetch_account(ctx, &ctx.pdas.pool);
    let position: reflex::Position = fetch_account(ctx, &ctx.pdas.position);
    let gross_yes = INITIAL_STAKE_AMOUNT + INITIAL_STAKE_AMOUNT / 2;
    let yes_fees = briber_fee(INITIAL_STAKE_AMOUNT) + briber_fee(INITIAL_STAKE_AMOUNT / 2);
    let no_fees = briber_fee(INITIAL_STAKE_AMOUNT);
    let expected_yes = gross_yes - yes_fees;
    let expected_no = INITIAL_STAKE_AMOUNT - no_fees;

    assert_eq!(pool.total_yes_staked, expected_yes);
    assert_eq!(pool.total_no_staked, expected_no);
    assert_eq!(position.authority, ctx.accounts.staker.pubkey());
    assert_eq!(position.yes_staked, expected_yes);
    assert_eq!(position.no_staked, expected_no);
    assert_eq!(pool.available_yes_fees, yes_fees);
    assert_eq!(pool.available_no_fees, no_fees);
    assert!(pool.yes_stake_seconds > 0);
    assert!(position.yes_stake_seconds > 0);
    assert_eq!(pool.yes_stake_seconds, position.yes_stake_seconds);
    assert_eq!(pool.no_stake_seconds, position.no_stake_seconds);
    assert_eq!(token_balance(ctx, &setup.yes.vault), gross_yes);
    assert_eq!(token_balance(ctx, &setup.no.vault), INITIAL_STAKE_AMOUNT);
    assert_eq!(
        token_balance(ctx, &setup.yes.ata),
        INITIAL_OUTCOME_AMOUNT - gross_yes
    );
    assert_eq!(
        token_balance(ctx, &setup.no.ata),
        INITIAL_OUTCOME_AMOUNT - INITIAL_STAKE_AMOUNT
    );
}

pub(super) fn assert_wrong_vault_rejected(ctx: &TestContext, setup: &PoolSetup) {
    let result = ctx
        .program
        .request()
        .accounts(reflex::accounts::DepositOutcome {
            staker: ctx.accounts.staker.pubkey(),
            pool: ctx.pdas.pool,
            market: ctx.market,
            position: ctx.pdas.position,
            outcome_mint: setup.yes.mint,
            pool_outcome_vault: setup.no.vault,
            staker_ata: setup.yes.ata,
            token_program: spl_token_interface::ID,
            system_program: solana_sdk_ids::system_program::id(),
        })
        .args(reflex::instruction::DepositOutcome {
            amount: INITIAL_STAKE_AMOUNT,
        })
        .payer(Rc::from(ctx.accounts.staker.insecure_clone()))
        .send();

    assert!(result.is_err(), "wrong outcome vault must be rejected");
}

pub(super) fn assert_zero_amount_rejected(ctx: &TestContext, setup: &PoolSetup) {
    let vault_before = token_balance(ctx, &setup.yes.vault);
    let result = run(ctx, &setup.yes, 0);

    assert!(result.is_err(), "zero-amount deposit must be rejected");
    assert_eq!(token_balance(ctx, &setup.yes.vault), vault_before);
}

fn token_balance(ctx: &TestContext, address: &Pubkey) -> u64 {
    let account = ctx
        .program
        .rpc()
        .get_account(address)
        .expect("failed to fetch token account");
    spl_token_interface::state::Account::unpack(&account.data)
        .expect("failed to decode token account")
        .amount
}
