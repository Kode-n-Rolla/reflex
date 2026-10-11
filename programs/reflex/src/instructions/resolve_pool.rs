use anchor_lang::prelude::*;

use crate::{constants::*, error::ReflexError, Config, Pool, PoolStatus};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub enum ResolutionOutcome {
    Yes,
    No,
}

#[derive(Accounts)]
pub struct ResolvePool<'info> {
    pub authority: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ ReflexError::Unauthorized
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [POOL_SEED, pool.market.as_ref()],
        bump = pool.bump,
        constraint = pool.can_settle() @ ReflexError::CanNotSettle
    )]
    pub pool: Account<'info, Pool>,
}

impl ResolvePool<'_> {
    pub fn proccess_ix(&mut self, outcome: ResolutionOutcome) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;

        self.pool.accrue_yes(now)?;
        self.pool.accrue_no(now)?;
        self.pool.resolution_timestamp = now;
        self.pool.status = match outcome {
            ResolutionOutcome::Yes => PoolStatus::Yes,
            ResolutionOutcome::No => PoolStatus::No,
        };

        Ok(())
    }
}
