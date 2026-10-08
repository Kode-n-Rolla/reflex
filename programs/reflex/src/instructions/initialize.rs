use anchor_lang::prelude::*;

use crate::{constants::*, Config};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = Config::SIZE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn proccess_ix(
        &mut self,
        fee_bps: u16,
        briber_fee_bps: u16,
        bumps: &InitializeBumps,
    ) -> Result<()> {
        self.config.set_inner(Config {
            authority: *self.authority.key,
            fee_bps,
            briber_fee_bps,
            bump: bumps.config,
        });

        Ok(())
    }
}
