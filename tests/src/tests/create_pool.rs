use std::rc::Rc;

use anchor_client::anchor_lang::prelude::program_pack::Pack;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

use crate::utils::{
    derive_outcome_vault, fetch_account, prepare_mint_and_ata, IncentiveSetup, OutcomeSetup,
    PoolSetup, TestContext, HARDCODED_MARKET, INITIAL_MINT_AMOUNT, INITIAL_POOL_FUNDING_AMOUNT,
    PROGRAM_ID, USDC_DECIMALS,
};

pub(super) fn run(ctx: &TestContext) -> PoolSetup {
    let creator = ctx.accounts.creator.pubkey();
    let incentive =
        prepare_mint_and_ata(&ctx.program, &creator, USDC_DECIMALS, INITIAL_MINT_AMOUNT);
    let yes = prepare_mint_and_ata(&ctx.program, &creator, USDC_DECIMALS, 0);
    let no = prepare_mint_and_ata(&ctx.program, &creator, USDC_DECIMALS, 0);

    create_mints(
        ctx,
        [&incentive.mint, &yes.mint, &no.mint],
        [&incentive, &yes, &no],
    );
    set_mock_market(&yes.mint.pubkey(), &no.mint.pubkey());

    let yes_vault = derive_outcome_vault(&PROGRAM_ID, &ctx.pdas.pool, &yes.mint.pubkey());
    let no_vault = derive_outcome_vault(&PROGRAM_ID, &ctx.pdas.pool, &no.mint.pubkey());

    let accounts = reflex::accounts::CreatePool {
        creator,
        config: ctx.pdas.config,
        market: HARDCODED_MARKET,
        pool: ctx.pdas.pool,
        incentive_mint: incentive.mint.pubkey(),
        yes_outcome_mint: yes.mint.pubkey(),
        no_outcome_mint: no.mint.pubkey(),
        creator_incentive_account: incentive.ata,
        pool_vault: ctx.pdas.pool_vault,
        yes_outcome_vault: yes_vault,
        no_outcome_vault: no_vault,
        token_program: spl_token_interface::ID,
        system_program: solana_sdk_ids::system_program::id(),
    };

    let zero_result = ctx
        .program
        .request()
        .accounts(accounts.clone())
        .args(reflex::instruction::CreatePool {
            incentive_amount: 0,
        })
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()))
        .send();
    assert!(zero_result.is_err(), "zero incentive must be rejected");

    let wrong_mints_result = ctx
        .program
        .request()
        .accounts(reflex::accounts::CreatePool {
            yes_outcome_mint: no.mint.pubkey(),
            no_outcome_mint: yes.mint.pubkey(),
            yes_outcome_vault: no_vault,
            no_outcome_vault: yes_vault,
            ..accounts.clone()
        })
        .args(reflex::instruction::CreatePool {
            incentive_amount: INITIAL_POOL_FUNDING_AMOUNT,
        })
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()))
        .send();
    assert!(
        wrong_mints_result.is_err(),
        "market outcome-mint mismatch must be rejected"
    );

    let wrong_source_result = ctx
        .program
        .request()
        .accounts(reflex::accounts::CreatePool {
            creator_incentive_account: yes.ata,
            ..accounts.clone()
        })
        .args(reflex::instruction::CreatePool {
            incentive_amount: INITIAL_POOL_FUNDING_AMOUNT,
        })
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()))
        .send();
    assert!(
        wrong_source_result.is_err(),
        "incentive source with the wrong mint must be rejected"
    );

    let wrong_vault_result = ctx
        .program
        .request()
        .accounts(reflex::accounts::CreatePool {
            pool_vault: incentive.ata,
            ..accounts.clone()
        })
        .args(reflex::instruction::CreatePool {
            incentive_amount: INITIAL_POOL_FUNDING_AMOUNT,
        })
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()))
        .send();
    assert!(
        wrong_vault_result.is_err(),
        "incentive vault with the wrong PDA must be rejected"
    );

    let signature = ctx
        .program
        .request()
        .accounts(accounts.clone())
        .args(reflex::instruction::CreatePool {
            incentive_amount: INITIAL_POOL_FUNDING_AMOUNT,
        })
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()))
        .send()
        .expect("create pool transaction failed");

    let duplicate_result = ctx
        .program
        .request()
        .accounts(accounts)
        .args(reflex::instruction::CreatePool {
            incentive_amount: INITIAL_POOL_FUNDING_AMOUNT,
        })
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()))
        .send();
    assert!(duplicate_result.is_err(), "duplicate pool must be rejected");

    println!("✅ Create pool transaction: {signature}");

    PoolSetup {
        incentive: IncentiveSetup {
            mint: incentive.mint.pubkey(),
            ata: incentive.ata,
        },
        yes: OutcomeSetup {
            mint: yes.mint.pubkey(),
            ata: yes.ata,
            vault: yes_vault,
        },
        no: OutcomeSetup {
            mint: no.mint.pubkey(),
            ata: no.ata,
            vault: no_vault,
        },
    }
}

pub(super) fn assert_created(ctx: &TestContext, setup: &PoolSetup) {
    let pool: reflex::Pool = fetch_account(ctx, &ctx.pdas.pool);

    assert_eq!(pool.creator, ctx.accounts.creator.pubkey());
    assert_eq!(pool.market, HARDCODED_MARKET);
    assert_eq!(pool.incentive_mint, setup.incentive.mint);
    assert_eq!(pool.yes_outcome_mint, setup.yes.mint);
    assert_eq!(pool.no_outcome_mint, setup.no.mint);
    assert_eq!(pool.total_incentive_amount, INITIAL_POOL_FUNDING_AMOUNT);
    assert_eq!(pool.total_yes_staked, 0);
    assert_eq!(pool.total_no_staked, 0);
    assert_eq!(pool.yes_stake_seconds, 0);
    assert_eq!(pool.no_stake_seconds, 0);
    assert_eq!(pool.available_yes_fees, 0);
    assert_eq!(pool.available_no_fees, 0);
    assert_eq!(pool.resolution_timestamp, 0);
    assert_eq!(pool.yes_last_update_ts, pool.creation_timestamp);
    assert_eq!(pool.no_last_update_ts, pool.creation_timestamp);
    assert_eq!(pool.fee_bps, 500);
    assert!(pool.is_open());
    assert_eq!(
        token_balance(ctx, &ctx.pdas.pool_vault),
        INITIAL_POOL_FUNDING_AMOUNT
    );
    assert_eq!(token_balance(ctx, &setup.yes.vault), 0);
    assert_eq!(token_balance(ctx, &setup.no.vault), 0);
}

fn create_mints(ctx: &TestContext, signers: [&Keypair; 3], setups: [&crate::utils::MintSetup; 3]) {
    let mut request = ctx
        .program
        .request()
        .payer(Rc::new(ctx.accounts.creator.insecure_clone()));

    for setup in setups {
        request = request
            .instruction(setup.create_account_ix.clone())
            .instruction(setup.init_mint_ix.clone())
            .instruction(setup.create_ata_ix.clone())
            .instruction(setup.mint_to_ix.clone());
    }

    for signer in signers {
        request = request.signer(signer);
    }

    request.send().expect("failed to create test mints");
}

fn set_mock_market(yes_mint: &Pubkey, no_mint: &Pubkey) {
    let mut data = vec![0_u8; reflex::constants::WORLD_MARKET_DATA_LEN];
    data[reflex::constants::WORLD_YES_MINT_OFFSET..reflex::constants::WORLD_NO_MINT_OFFSET]
        .copy_from_slice(yes_mint.as_ref());
    data[reflex::constants::WORLD_NO_MINT_OFFSET..reflex::constants::WORLD_NO_MINT_OFFSET + 32]
        .copy_from_slice(no_mint.as_ref());
    let data = data
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    let response: Value = Client::new()
        .post("http://127.0.0.1:8899")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "surfnet_setAccount",
            "params": [
                HARDCODED_MARKET.to_string(),
                {
                    "lamports": 10_000_000,
                    "data": data,
                    "owner": solana_sdk_ids::system_program::id().to_string(),
                    "executable": false
                }
            ]
        }))
        .send()
        .expect("failed to call surfnet_setAccount")
        .json()
        .expect("invalid surfnet_setAccount response");

    assert!(
        response.get("error").is_none(),
        "surfnet_setAccount failed: {response}"
    );
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
