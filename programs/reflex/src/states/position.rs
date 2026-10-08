use anchor_lang::prelude::*;

use crate::{error::ReflexError, DISCRIMINATOR_SIZE};

#[account]
#[derive(InitSpace)]
pub struct Position {
    pub yes_stake_seconds: u128,
    pub no_stake_seconds: u128,
    pub yes_staked: u64,
    pub no_staked: u64,
    pub yes_last_update_ts: i64,
    pub no_last_update_ts: i64,
    pub authority: Pubkey,
    pub bump: u8,
}

impl Position {
    pub const SIZE: usize = DISCRIMINATOR_SIZE + Position::INIT_SPACE;

    pub fn accrue_yes(&mut self, now: i64) -> Result<()> {
        let dt = now
            .checked_sub(self.yes_last_update_ts)
            .ok_or(ReflexError::MathOverflow)?;

        require!(dt >= 0, ReflexError::Unauthorized);

        let delta = (self.yes_staked as u128)
            .checked_mul(dt as u128)
            .ok_or(ReflexError::MathOverflow)?;

        self.yes_stake_seconds = self
            .yes_stake_seconds
            .checked_add(delta)
            .ok_or(ReflexError::MathOverflow)?;

        self.yes_last_update_ts = now;

        Ok(())
    }

    pub fn accrue_no(&mut self, now: i64) -> Result<()> {
        let dt = now
            .checked_sub(self.no_last_update_ts)
            .ok_or(ReflexError::MathOverflow)?;

        require!(dt >= 0, ReflexError::Unauthorized);

        let delta = (self.no_staked as u128)
            .checked_mul(dt as u128)
            .ok_or(ReflexError::MathOverflow)?;

        self.no_stake_seconds = self
            .no_stake_seconds
            .checked_add(delta)
            .ok_or(ReflexError::MathOverflow)?;

        self.no_last_update_ts = now;

        Ok(())
    }

    pub fn add_yes_staked(&mut self, amount: u64) -> Result<()> {
        self.yes_staked = self
            .yes_staked
            .checked_add(amount)
            .ok_or(ReflexError::MathOverflow)?;

        Ok(())
    }

    pub fn add_no_staked(&mut self, amount: u64) -> Result<()> {
        self.no_staked = self
            .no_staked
            .checked_add(amount)
            .ok_or(ReflexError::MathOverflow)?;

        Ok(())
    }
}
