use std::rc::Rc;

use solana_signer::Signer;

use crate::utils::{OutcomeSetup, TestContext, HARDCODED_MARKET, PREDICT_PROGRAM_ID};

pub(super) fn run(ctx: &TestContext, outcome: &OutcomeSetup, amount: u64) -> Result<(), String> {
    let signature = ctx
        .program
        .request()
        .accounts(reflex::accounts::DepositOutcome {
            staker: ctx.accounts.staker.pubkey(),
            pool: ctx.pdas.pool,
            market: HARDCODED_MARKET,
            position: ctx.pdas.position,
            outcome_mint: outcome.mint,
            pool_outcome_vault: outcome.vault,
            staker_ata: outcome.ata,
            predict_program: PREDICT_PROGRAM_ID,
            token_program: spl_token_interface::ID,
            system_program: solana_sdk_ids::system_program::id(),
        })
        .args(reflex::instruction::DepositOutcome { amount })
        .payer(Rc::from(ctx.accounts.staker.insecure_clone()))
        .send()
        .map_err(|error| error.to_string())?;

    println!("✅ Stake outcome transaction: {signature}");
    Ok(())
}
