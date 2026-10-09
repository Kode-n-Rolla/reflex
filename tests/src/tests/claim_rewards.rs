use std::rc::Rc;

use anchor_client::{anchor_lang::prelude::program_pack::Pack, CommitmentConfig};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_associated_token_account_interface::address::get_associated_token_address;

use crate::utils::{fetch_account, PoolSetup, TestContext};

pub(super) fn run(ctx: &TestContext, setup: &PoolSetup) {
    run_for(
        ctx,
        setup,
        &ctx.accounts.staker,
        ctx.pdas.position,
        setup.yes.ata,
        setup.no.ata,
        true,
    );
}

pub(super) fn run_for(
    ctx: &TestContext,
    setup: &PoolSetup,
    staker_keypair: &Keypair,
    position_address: Pubkey,
    yes_ata: Pubkey,
    no_ata: Pubkey,
    expect_empty_outcome_vaults: bool,
) -> u64 {
    let staker = staker_keypair.pubkey();
    let staker_incentive_ata = get_associated_token_address(&staker, &setup.incentive.mint);
    let pool: reflex::Pool = fetch_account(ctx, &ctx.pdas.pool);
    let position: reflex::Position = fetch_account(ctx, &position_address);

    let (stake_seconds, staked, last_update_ts, total_stake_seconds) = if pool.is_yes() {
        (
            position.yes_stake_seconds,
            position.yes_staked,
            position.yes_last_update_ts,
            pool.yes_stake_seconds,
        )
    } else if pool.is_no() {
        (
            position.no_stake_seconds,
            position.no_staked,
            position.no_last_update_ts,
            pool.no_stake_seconds,
        )
    } else {
        panic!("claim test requires a settled pool");
    };
    let elapsed = pool
        .resolution_timestamp
        .checked_sub(last_update_ts)
        .expect("resolution timestamp precedes the position update");
    let winning_stake_seconds = stake_seconds
        .checked_add(
            u128::from(staked)
                .checked_mul(u128::try_from(elapsed).expect("negative elapsed time"))
                .expect("winning stake-seconds overflow"),
        )
        .expect("winning stake-seconds overflow");
    let expected_reward = if total_stake_seconds == 0 {
        0
    } else {
        winning_stake_seconds
            .checked_mul(u128::from(pool.total_incentive_amount))
            .expect("reward calculation overflow")
            .checked_div(total_stake_seconds)
            .expect("pool has no winning stake-seconds") as u64
    };

    let yes_before = token_balance(ctx, &yes_ata);
    let no_before = token_balance(ctx, &no_ata);

    let accounts = claim_accounts_for(ctx, setup, staker, position_address, yes_ata, no_ata);

    let signature = ctx
        .program
        .request()
        .accounts(accounts.clone())
        .args(reflex::instruction::ClaimRewards {})
        .payer(Rc::new(staker_keypair.insecure_clone()))
        .send()
        .expect("claim rewards transaction failed");

    assert_eq!(
        token_balance(ctx, &yes_ata) - yes_before,
        position.yes_staked
    );
    assert_eq!(token_balance(ctx, &no_ata) - no_before, position.no_staked);
    assert_eq!(token_balance(ctx, &staker_incentive_ata), expected_reward);
    if expect_empty_outcome_vaults {
        assert_eq!(token_balance(ctx, &setup.yes.vault), 0);
        assert_eq!(token_balance(ctx, &setup.no.vault), 0);
    }

    let position_account = ctx
        .program
        .rpc()
        .get_account_with_commitment(&position_address, CommitmentConfig::confirmed())
        .expect("failed to check whether position was closed")
        .value;
    assert!(position_account.is_none(), "claim must close the position");

    let second_claim = ctx
        .program
        .request()
        .accounts(accounts)
        .args(reflex::instruction::ClaimRewards {})
        .payer(Rc::new(staker_keypair.insecure_clone()))
        .send();
    assert!(second_claim.is_err(), "second claim must be rejected");

    println!("✅ Claim rewards transaction: {signature}");
    expected_reward
}

pub(super) fn assert_before_resolution_rejected(ctx: &TestContext, setup: &PoolSetup) {
    let result = ctx
        .program
        .request()
        .accounts(claim_accounts(ctx, setup, ctx.accounts.staker.pubkey()))
        .args(reflex::instruction::ClaimRewards {})
        .payer(Rc::new(ctx.accounts.staker.insecure_clone()))
        .send();

    assert!(result.is_err(), "claim before resolution must be rejected");
}

pub(super) fn assert_wrong_signer_rejected(ctx: &TestContext, setup: &PoolSetup) {
    let result = ctx
        .program
        .request()
        .accounts(claim_accounts(ctx, setup, ctx.accounts.authority.pubkey()))
        .args(reflex::instruction::ClaimRewards {})
        .send();

    assert!(
        result.is_err(),
        "claim by the wrong signer must be rejected"
    );
}

pub(super) fn assert_wrong_vault_rejected(ctx: &TestContext, setup: &PoolSetup) {
    let mut accounts = claim_accounts(ctx, setup, ctx.accounts.staker.pubkey());
    accounts.yes_outcome_vault = setup.no.vault;

    let result = ctx
        .program
        .request()
        .accounts(accounts)
        .args(reflex::instruction::ClaimRewards {})
        .payer(Rc::new(ctx.accounts.staker.insecure_clone()))
        .send();

    assert!(
        result.is_err(),
        "claim with the wrong vault must be rejected"
    );
}

pub(super) fn assert_shortfall_rejected_atomically(ctx: &TestContext, setup: &PoolSetup) {
    let yes_before = token_balance(ctx, &setup.yes.ata);
    let no_before = token_balance(ctx, &setup.no.ata);
    let incentive_vault_before = token_balance(ctx, &ctx.pdas.pool_vault);
    let yes_vault_before = token_balance(ctx, &setup.yes.vault);
    let no_vault_before = token_balance(ctx, &setup.no.vault);

    let result = ctx
        .program
        .request()
        .accounts(claim_accounts(ctx, setup, ctx.accounts.staker.pubkey()))
        .args(reflex::instruction::ClaimRewards {})
        .payer(Rc::new(ctx.accounts.staker.insecure_clone()))
        .send();

    assert!(result.is_err(), "claim with insufficient custody must fail");
    assert_eq!(token_balance(ctx, &setup.yes.ata), yes_before);
    assert_eq!(token_balance(ctx, &setup.no.ata), no_before);
    assert_eq!(
        token_balance(ctx, &ctx.pdas.pool_vault),
        incentive_vault_before
    );
    assert_eq!(token_balance(ctx, &setup.yes.vault), yes_vault_before);
    assert_eq!(token_balance(ctx, &setup.no.vault), no_vault_before);
    assert!(
        ctx.program.rpc().get_account(&ctx.pdas.position).is_ok(),
        "failed claim must not close the position"
    );
}

fn claim_accounts(
    ctx: &TestContext,
    setup: &PoolSetup,
    staker: Pubkey,
) -> reflex::accounts::ClaimRewards {
    claim_accounts_for(
        ctx,
        setup,
        staker,
        ctx.pdas.position,
        setup.yes.ata,
        setup.no.ata,
    )
}

fn claim_accounts_for(
    ctx: &TestContext,
    setup: &PoolSetup,
    staker: Pubkey,
    position: Pubkey,
    yes_ata: Pubkey,
    no_ata: Pubkey,
) -> reflex::accounts::ClaimRewards {
    reflex::accounts::ClaimRewards {
        staker,
        pool: ctx.pdas.pool,
        market: ctx.market,
        position,
        incentive_mint: setup.incentive.mint,
        yes_outcome_mint: setup.yes.mint,
        no_outcome_mint: setup.no.mint,
        pool_vault: ctx.pdas.pool_vault,
        yes_outcome_vault: setup.yes.vault,
        no_outcome_vault: setup.no.vault,
        staker_incentive_ata: get_associated_token_address(&staker, &setup.incentive.mint),
        staker_yes_ata: yes_ata,
        staker_no_ata: no_ata,
        associated_token_program: spl_associated_token_account_interface::program::ID,
        token_program: spl_token_interface::ID,
        system_program: solana_sdk_ids::system_program::id(),
    }
}

pub(super) fn token_balance(ctx: &TestContext, address: &Pubkey) -> u64 {
    let account = ctx
        .program
        .rpc()
        .get_account(address)
        .expect("failed to fetch token account");
    spl_token_interface::state::Account::unpack(&account.data)
        .expect("failed to decode token account")
        .amount
}
