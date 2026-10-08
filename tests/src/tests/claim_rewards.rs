use std::rc::Rc;

use anchor_client::CommitmentConfig;
use solana_signer::Signer;
use spl_associated_token_account_interface::address::get_associated_token_address;

use crate::utils::{
    fetch_account, IncentiveSetup, OutcomeSetup, TestContext, HARDCODED_MARKET, PREDICT_PROGRAM_ID,
};

pub(super) fn run(ctx: &TestContext, incentive: &IncentiveSetup, yes: &OutcomeSetup) {
    let staker = ctx.accounts.staker.pubkey();
    let staker_incentive_ata = get_associated_token_address(&staker, &incentive.mint);
    let yes_tokens_before = token_balance(ctx, &yes.ata);
    let pool: reflex::Pool = fetch_account(ctx, &ctx.pdas.pool);
    let position: reflex::Position = fetch_account(ctx, &ctx.pdas.position);
    assert!(pool.is_yes(), "claim test requires a settled YES pool");
    let elapsed = pool
        .resolution_timestamp
        .checked_sub(position.yes_last_update_ts)
        .expect("resolution timestamp precedes the position update");
    let yes_stake_seconds = position
        .yes_stake_seconds
        .checked_add(
            u128::from(position.yes_staked)
                .checked_mul(u128::try_from(elapsed).expect("negative elapsed time"))
                .expect("YES stake-seconds overflow"),
        )
        .expect("YES stake-seconds overflow");
    let expected_reward = yes_stake_seconds
        .checked_mul(u128::from(pool.total_incentive_amount))
        .expect("reward calculation overflow")
        .checked_div(pool.yes_stake_seconds)
        .expect("pool has no YES stake-seconds");

    let signature = ctx
        .program
        .request()
        .accounts(reflex::accounts::ClaimRewards {
            staker,
            pool: ctx.pdas.pool,
            market: HARDCODED_MARKET,
            position: ctx.pdas.position,
            mint: incentive.mint,
            outcome_mint: yes.mint,
            pool_vault: ctx.pdas.pool_vault,
            pool_outcome_vault: yes.vault,
            staker_ata: staker_incentive_ata,
            staker_outcome_ata: yes.ata,
            predict_program: PREDICT_PROGRAM_ID,
            token_program: spl_token_interface::ID,
            associated_token_program: spl_associated_token_account_interface::program::ID,
            system_program: solana_sdk_ids::system_program::id(),
        })
        .args(reflex::instruction::ClaimRewards {})
        .payer(Rc::from(ctx.accounts.staker.insecure_clone()))
        .send()
        .expect("claim rewards transaction failed");

    let yes_tokens_after = token_balance(ctx, &yes.ata);
    let incentive_tokens_after = token_balance(ctx, &staker_incentive_ata);

    assert_eq!(
        yes_tokens_after - yes_tokens_before,
        position.yes_staked,
        "claim should return the staker's winning YES tokens"
    );
    assert_eq!(
        incentive_tokens_after,
        u64::try_from(expected_reward).expect("reward does not fit in u64"),
        "claim should pay the staker's share of incentives"
    );
    let position_account = ctx
        .program
        .rpc()
        .get_account_with_commitment(&ctx.pdas.position, CommitmentConfig::confirmed())
        .expect("failed to check whether position was closed")
        .value;
    assert!(
        position_account.is_none(),
        "claim should close the staker's position"
    );

    println!("✅ Claim rewards transaction: {signature}");
}

fn token_balance(ctx: &TestContext, address: &solana_pubkey::Pubkey) -> u64 {
    use anchor_client::anchor_lang::prelude::program_pack::Pack;

    let account = ctx
        .program
        .rpc()
        .get_account(address)
        .expect("failed to fetch token account");
    spl_token_interface::state::Account::unpack(&account.data)
        .expect("failed to decode token account")
        .amount
}
