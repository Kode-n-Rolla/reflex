use std::rc::Rc;

use solana_signer::Signer;

use crate::utils::TestContext;

pub(super) fn run(ctx: &TestContext) {
    let accounts = reflex::accounts::Initialize {
        authority: ctx.accounts.authority.pubkey(),
        config: ctx.pdas.config,
        system_program: solana_sdk_ids::system_program::id(),
    };
    let nonzero_protocol_fee = ctx
        .program
        .request()
        .accounts(accounts.clone())
        .args(reflex::instruction::Initialize {
            fee_bps: 1,
            briber_fee_bps: 500,
        })
        .send();
    assert!(
        nonzero_protocol_fee.is_err(),
        "nonzero protocol fee must be rejected"
    );

    let excessive_briber_fee = ctx
        .program
        .request()
        .accounts(accounts.clone())
        .args(reflex::instruction::Initialize {
            fee_bps: 0,
            briber_fee_bps: 10_001,
        })
        .payer(Rc::new(ctx.accounts.authority.insecure_clone()))
        .send();
    assert!(
        excessive_briber_fee.is_err(),
        "briber fee above 10000 bps must be rejected"
    );

    let signature = ctx
        .program
        .request()
        .accounts(accounts)
        .args(reflex::instruction::Initialize {
            fee_bps: 0,
            briber_fee_bps: 500,
        })
        .send()
        .expect("initialize transaction failed");

    println!("✅ Initialize transaction: {signature}");
}
