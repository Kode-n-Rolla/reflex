use std::rc::Rc;

use solana_signer::Signer;
use spl_associated_token_account_interface::address::get_associated_token_address;

use crate::utils::{fetch_account, PoolSetup, TestContext};

pub(super) fn run(ctx: &TestContext, setup: &PoolSetup) {
    let pool: reflex::Pool = fetch_account(ctx, &ctx.pdas.pool);
    let creator_yes_ata = get_associated_token_address(&pool.creator, &setup.yes.mint);
    let creator_no_ata = get_associated_token_address(&pool.creator, &setup.no.mint);
    let yes_before = super::claim_rewards::token_balance(ctx, &creator_yes_ata);
    let no_before = super::claim_rewards::token_balance(ctx, &creator_no_ata);

    let accounts = reflex::accounts::WithdrawBriberFees {
        creator: pool.creator,
        pool: ctx.pdas.pool,
        yes_outcome_mint: setup.yes.mint,
        no_outcome_mint: setup.no.mint,
        yes_outcome_vault: setup.yes.vault,
        no_outcome_vault: setup.no.vault,
        creator_yes_ata,
        creator_no_ata,
        token_program: spl_token_interface::ID,
    };

    let unauthorized = ctx
        .program
        .request()
        .accounts(reflex::accounts::WithdrawBriberFees {
            creator: ctx.accounts.staker.pubkey(),
            ..accounts.clone()
        })
        .args(reflex::instruction::WithdrawBriberFees {})
        .payer(Rc::new(ctx.accounts.staker.insecure_clone()))
        .send();
    assert!(unauthorized.is_err(), "non-creator withdrawal must fail");

    let signature = ctx
        .program
        .request()
        .accounts(accounts.clone())
        .args(reflex::instruction::WithdrawBriberFees {})
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()))
        .send()
        .expect("creator fee withdrawal failed");

    assert_eq!(
        super::claim_rewards::token_balance(ctx, &creator_yes_ata) - yes_before,
        pool.available_yes_fees
    );
    assert_eq!(
        super::claim_rewards::token_balance(ctx, &creator_no_ata) - no_before,
        pool.available_no_fees
    );
    assert_eq!(
        super::claim_rewards::token_balance(ctx, &setup.yes.vault),
        0
    );
    assert_eq!(super::claim_rewards::token_balance(ctx, &setup.no.vault), 0);

    let updated_pool: reflex::Pool = fetch_account(ctx, &ctx.pdas.pool);
    assert_eq!(updated_pool.available_yes_fees, 0);
    assert_eq!(updated_pool.available_no_fees, 0);

    let repeated = ctx
        .program
        .request()
        .accounts(accounts)
        .args(reflex::instruction::WithdrawBriberFees {})
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()))
        .send();
    assert!(repeated.is_err(), "repeated fee withdrawal must fail");

    println!("✅ Withdraw briber fees transaction: {signature}");
}
