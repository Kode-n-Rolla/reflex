use anchor_lang::constant;
use anchor_lang::prelude::Pubkey;
use anchor_lang::pubkey;

pub const DISCRIMINATOR_SIZE: usize = 8;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const POOL_SEED: &[u8] = b"pool";

#[constant]
pub const VAULT_SEED: &[u8] = b"vault";

#[constant]
pub const POSITION_SEED: &[u8] = b"position";

pub const WORLD_MARKET_DATA_LEN: usize = 320;
pub const WORLD_YES_MINT_OFFSET: usize = 72;
pub const WORLD_NO_MINT_OFFSET: usize = 104;

pub const BPS_DENOMINATOR: u64 = 10_000;
pub const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const WSOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
