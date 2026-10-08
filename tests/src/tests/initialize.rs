use solana_signer::Signer;

use crate::utils::TestContext;

pub(super) fn run(ctx: &TestContext) {
    let signature = ctx
        .program
        .request()
        .accounts(reflex::accounts::Initialize {
            authority: ctx.accounts.authority.pubkey(),
            config: ctx.pdas.config,
            system_program: solana_sdk_ids::system_program::id(),
        })
        .args(reflex::instruction::Initialize {
            fee_bps: 500,
            briber_fee_bps: 500,
        })
        .send()
        .expect("initialize transaction failed");

    println!("✅ Initialize transaction: {signature}");
}
