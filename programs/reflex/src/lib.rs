pub mod constants;
pub mod error;
pub mod instructions;
pub mod states;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use states::*;

declare_id!("E3GJVnGuMY187ztijNmnTKDwLV4avi4AcF9gCxRiYYWg");

#[program]
pub mod reflex {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, fee_bps: u16, briber_fee_bps: u16) -> Result<()> {
        ctx.accounts
            .proccess_ix(fee_bps, briber_fee_bps, &ctx.bumps)
    }

    pub fn create_pool(ctx: Context<CreatePool>, incentive_amount: u64) -> Result<()> {
        ctx.accounts.proccess_ix(incentive_amount, &ctx.bumps)
    }

    pub fn deposit_outcome(ctx: Context<DepositOutcome>, amount: u64) -> Result<()> {
        ctx.accounts.proccess_ix(amount, &ctx.bumps)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        ctx.accounts.proccess_ix()
    }
}
