use std::rc::Rc;

use solana_signer::Signer;

use crate::utils::{fetch_account, TestContext};

pub(super) fn run(ctx: &TestContext, outcome: reflex::ResolutionOutcome) {
    let signature = ctx
        .program
        .request()
        .accounts(reflex::accounts::ResolvePool {
            authority: ctx.accounts.authority.pubkey(),
            config: ctx.pdas.config,
            pool: ctx.pdas.pool,
        })
        .args(reflex::instruction::ResolvePool { outcome })
        .send()
        .expect("resolve pool transaction failed");

    println!("✅ Resolve pool transaction: {signature}");
}

pub(super) fn assert_unauthorized_rejected(ctx: &TestContext) {
    let result = ctx
        .program
        .request()
        .accounts(reflex::accounts::ResolvePool {
            authority: ctx.accounts.staker.pubkey(),
            config: ctx.pdas.config,
            pool: ctx.pdas.pool,
        })
        .args(reflex::instruction::ResolvePool {
            outcome: reflex::ResolutionOutcome::Yes,
        })
        .payer(Rc::new(ctx.accounts.staker.insecure_clone()))
        .send();

    assert!(result.is_err(), "unauthorized resolution must be rejected");
}

pub(super) fn assert_resolved(ctx: &TestContext, outcome: reflex::ResolutionOutcome) {
    let pool: reflex::Pool = fetch_account(ctx, &ctx.pdas.pool);

    match outcome {
        reflex::ResolutionOutcome::Yes => assert!(pool.is_yes()),
        reflex::ResolutionOutcome::No => assert!(pool.is_no()),
    }
    assert!(pool.resolution_timestamp > 0);
    assert_eq!(pool.yes_last_update_ts, pool.resolution_timestamp);
    assert_eq!(pool.no_last_update_ts, pool.resolution_timestamp);
    if pool.total_yes_staked > 0 {
        assert!(pool.yes_stake_seconds > 0);
    } else {
        assert_eq!(pool.yes_stake_seconds, 0);
    }
    if pool.total_no_staked > 0 {
        assert!(pool.no_stake_seconds > 0);
    } else {
        assert_eq!(pool.no_stake_seconds, 0);
    }
}

pub(super) fn assert_second_resolution_rejected(ctx: &TestContext) {
    let result = ctx
        .program
        .request()
        .accounts(reflex::accounts::ResolvePool {
            authority: ctx.accounts.authority.pubkey(),
            config: ctx.pdas.config,
            pool: ctx.pdas.pool,
        })
        .args(reflex::instruction::ResolvePool {
            outcome: reflex::ResolutionOutcome::No,
        })
        .send();

    assert!(result.is_err(), "second resolution must be rejected");
}
