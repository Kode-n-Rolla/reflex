use anchor_lang::constant;

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
