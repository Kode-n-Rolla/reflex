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
