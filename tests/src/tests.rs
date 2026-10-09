mod claim_rewards;
mod create_pool;
mod deposit_outcome;
mod initialize;

use std::time::{SystemTime, UNIX_EPOCH};

use anchor_client::anchor_lang::prelude::clock::SECONDS_PER_DAY;
use reqwest::blocking::Client;
use serde_json::json;
use solana_signer::Signer;

use crate::utils::{fetch_account, setup, TestContext};

#[test]
fn test_reflex_lifecycle() {
    let ctx = setup();

    initialize::run(&ctx);
    assert_initialized(&ctx);

    let pool = create_pool::run(&ctx);
    create_pool::assert_created(&ctx, &pool);

    // deposit_outcome::run(&ctx, &pool.yes, INITIAL_STAKE_AMOUNT).expect("initial YES stake failed");
    // deposit_outcome::run(&ctx, &pool.no, INITIAL_STAKE_AMOUNT).expect("initial NO stake failed");

    advance_time(7);

    // deposit_outcome::run(&ctx, &pool.yes, INITIAL_STAKE_AMOUNT / 2)
    //     .expect("additional YES stake before settlement failed");

    // settle pool

    // claim_rewards::run(&ctx, &pool.incentive, &pool.yes);

    // assert!(
    //     stake_outcome::run(&ctx, &pool.no, INITIAL_STAKE_AMOUNT / 2).is_err(),
    //     "staking should fail after the pool is settled"
    // );
}

fn assert_initialized(ctx: &TestContext) {
    let config: reflex::Config = fetch_account(ctx, &ctx.pdas.config);

    assert_eq!(config.authority, ctx.accounts.authority.pubkey());
    assert_eq!(config.fee_bps, 500);
    assert_eq!(config.briber_fee_bps, 500);
}

fn advance_time(days: i64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let absolute_timestamp = now + (days * SECONDS_PER_DAY as i64 * 1_000);

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
