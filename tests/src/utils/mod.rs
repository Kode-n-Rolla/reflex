use std::{path::PathBuf, rc::Rc};

use anchor_client::{
    anchor_lang::prelude::{program_pack::Pack, system_instruction::create_account},
    Client, Cluster, CommitmentConfig, Instruction, Program,
};
use reqwest::blocking::Client as HttpClient;
use serde_json::{json, Value};

use solana_keypair::{read_keypair_file, Keypair};
use solana_pubkey::{pubkey, Pubkey};
use solana_signer::Signer;
use spl_associated_token_account_interface::{
    address::get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token_interface::instruction::{initialize_mint2, mint_to};

// -----------------------------------------------------------------------------
// constants
// -----------------------------------------------------------------------------
pub const PROGRAM_ID: Pubkey = pubkey!("E3GJVnGuMY187ztijNmnTKDwLV4avi4AcF9gCxRiYYWg");
pub const HARDCODED_MARKET: Pubkey = pubkey!("JDryy7Dowir9XXAaW2Gyosc5Hdm2hRv1KyU4HC3wR92B");

pub const ONE_SOL: u64 = 1_000_000_000;
pub const USDC_DECIMALS: u8 = 6;
pub const INITIAL_MINT_AMOUNT: u64 = 10_000_000_000; // 10k USDC
pub const INITIAL_OUTCOME_AMOUNT: u64 = 5_000_000_000;
pub const INITIAL_STAKE_AMOUNT: u64 = 1_000_000_000;
pub const INITIAL_POOL_FUNDING_AMOUNT: u64 = 1_000_000_000;

// -----------------------------------------------------------------------------
// structs
// -----------------------------------------------------------------------------
pub struct TestAccounts {
    pub authority: Keypair,
    pub creator: Keypair,
    pub staker: Keypair,
}

pub struct TestPdas {
    pub config: Pubkey,
    pub pool: Pubkey,
    pub pool_vault: Pubkey,
    pub position: Pubkey,
}

pub struct TestContext {
    pub client: Client<Rc<Keypair>>,
    pub program: Program<Rc<Keypair>>,
    pub accounts: TestAccounts,
    pub pdas: TestPdas,
    pub market: Pubkey,
}

pub struct MintSetup {
    pub mint: Keypair,
    pub ata: Pubkey,
    pub create_account_ix: Instruction,
    pub init_mint_ix: Instruction,
    pub create_ata_ix: Instruction,
    pub mint_to_ix: Instruction,
}

pub struct OutcomeSetup {
    pub mint: Pubkey,
    pub ata: Pubkey,
    pub vault: Pubkey,
}

pub struct IncentiveSetup {
    pub mint: Pubkey,
    pub ata: Pubkey,
}

pub struct PoolSetup {
    pub incentive: IncentiveSetup,
    pub yes: OutcomeSetup,
    pub no: OutcomeSetup,
}

pub fn fetch_account<T: anchor_client::anchor_lang::AccountDeserialize>(
    ctx: &TestContext,
    address: &Pubkey,
) -> T {
    let account = ctx
        .program
        .rpc()
        .get_account(address)
        .expect("failed to fetch account");
    let mut data: &[u8] = &account.data;

    T::try_deserialize(&mut data).expect("failed to deserialize account")
}

pub fn derive_outcome_vault(program_id: &Pubkey, pool: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[reflex::constants::VAULT_SEED, pool.as_ref(), mint.as_ref()],
        program_id,
    )
    .0
}

// -----------------------------------------------------------------------------
// utils fns
// -----------------------------------------------------------------------------

fn get_keypair_path(filename: &str) -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());

    PathBuf::from(manifest_dir).join("keypairs").join(filename)
}

pub fn load_accounts() -> TestAccounts {
    let anchor_wallet = std::env::var("ANCHOR_WALLET")
        .expect("ANCHOR_WALLET must be set to the authority keypair path");

    let authority = read_keypair_file(&anchor_wallet).unwrap_or_else(|error| {
        panic!("failed to read authority keypair {anchor_wallet}: {error}")
    });

    let creator_path = get_keypair_path("creator.json");
    let staker_path = get_keypair_path("staker.json");

    let creator = read_keypair_file(&creator_path)
        .unwrap_or_else(|error| panic!("failed to read creator keypair: {error}"));
    let staker = read_keypair_file(&staker_path)
        .unwrap_or_else(|error| panic!("failed to read staker keypair: {error}"));

    TestAccounts {
        authority,
        creator,
        staker,
    }
}

pub fn derive_pdas(program_id: &Pubkey, market: &Pubkey, staker_address: &Pubkey) -> TestPdas {
    let (config, _) = Pubkey::find_program_address(&[reflex::constants::CONFIG_SEED], program_id);
    let (pool, _) =
        Pubkey::find_program_address(&[reflex::constants::POOL_SEED, market.as_ref()], program_id);
    let (pool_vault, _) =
        Pubkey::find_program_address(&[reflex::constants::VAULT_SEED, pool.as_ref()], program_id);
    let (position, _) = Pubkey::find_program_address(
        &[
            reflex::constants::POSITION_SEED,
            pool.as_ref(),
            staker_address.as_ref(),
        ],
        program_id,
    );

    TestPdas {
        config,
        pool,
        pool_vault,
        position,
    }
}

// -----------------------------------------------------------------------------
// initial setup fn
// -----------------------------------------------------------------------------
pub fn setup(market: Pubkey) -> TestContext {
    let accounts = load_accounts();
    let pdas = derive_pdas(&PROGRAM_ID, &market, &accounts.staker.pubkey());

    let authority_rc = Rc::new(accounts.authority.insecure_clone());
    let client = Client::new_with_options(
        Cluster::Localnet,
        authority_rc,
        CommitmentConfig::confirmed(),
    );

    let program = client
        .program(PROGRAM_ID)
        .expect("Error al instanciar el cliente del programa");

    println!("{}", program.rpc().url());

    fund_account(&accounts.authority.pubkey());
    fund_account(&accounts.creator.pubkey());
    fund_account(&accounts.staker.pubkey());

    TestContext {
        client,
        program,
        accounts,
        pdas,
        market,
    }
}

pub(super) fn fund_account(address: &Pubkey) {
    let response: Value = HttpClient::new()
        .post("http://127.0.0.1:8899")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "surfnet_setAccount",
            "params": [
                address.to_string(),
                {
                    "lamports": ONE_SOL,
                    "owner": solana_sdk_ids::system_program::id().to_string(),
                    "executable": false
                }
            ]
        }))
        .send()
        .expect("failed to fund test account")
        .json()
        .expect("invalid surfnet_setAccount response");

    assert!(
        response.get("error").is_none(),
        "surfnet_setAccount failed: {response}"
    );
}

pub fn set_token_balance(ctx: &TestContext, address: &Pubkey, amount: u64) {
    let mut account = ctx
        .program
        .rpc()
        .get_account(address)
        .expect("failed to fetch token account for balance override");
    account.data[64..72].copy_from_slice(&amount.to_le_bytes());
    let data = account
        .data
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    let response: Value = HttpClient::new()
        .post("http://127.0.0.1:8899")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "surfnet_setAccount",
            "params": [
                address.to_string(),
                {
                    "lamports": account.lamports,
                    "data": data,
                    "owner": account.owner.to_string(),
                    "executable": account.executable
                }
            ]
        }))
        .send()
        .expect("failed to override token balance")
        .json()
        .expect("invalid token balance override response");

    assert!(
        response.get("error").is_none(),
        "surfnet_setAccount failed: {response}"
    );
}

// -----------------------------------------------------------------------------
// token helpers
// -----------------------------------------------------------------------------
pub fn prepare_mint_and_ata(
    program: &Program<Rc<Keypair>>,
    owner: &Pubkey,
    decimals: u8,
    amount: u64,
) -> MintSetup {
    let mint = Keypair::new();
    let token_program = spl_token_interface::ID;

    let lamports = program
        .rpc()
        .get_minimum_balance_for_rent_exemption(spl_token_interface::state::Mint::LEN)
        .expect("Error obteniendo rent exemption");

    let create_account_ix = create_account(
        owner,
        &mint.pubkey(),
        lamports,
        spl_token_interface::state::Mint::LEN as u64,
        &token_program,
    );

    let init_mint_ix = initialize_mint2(&token_program, &mint.pubkey(), owner, None, decimals)
        .expect("Error creando instrucción initialize_mint2");

    let ata = get_associated_token_address(owner, &mint.pubkey());
    let create_ata_ix =
        create_associated_token_account(owner, owner, &mint.pubkey(), &token_program);

    let mint_to_ix = mint_to(
        &token_program,
        &mint.pubkey(),
        &ata,
        owner,
        &[owner],
        amount,
    )
    .expect("Error creando instrucción mint_to");

    MintSetup {
        mint,
        ata,
        create_account_ix,
        init_mint_ix,
        create_ata_ix,
        mint_to_ix,
    }
}
