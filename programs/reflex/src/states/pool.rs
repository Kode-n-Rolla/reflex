use anchor_lang::prelude::*;

use crate::{error::ReflexError, DISCRIMINATOR_SIZE};

#[account]
#[derive(InitSpace)]
pub struct Pool {
    pub creator: Pubkey,
    pub incentive_mint: Pubkey,
    pub yes_outcome_mint: Pubkey,
    pub no_outcome_mint: Pubkey,

    pub yes_stake_seconds: u128, // global accumulator
    pub no_stake_seconds: u128,  // global accumulator

    pub total_incentive_amount: u64,
    pub total_yes_staked: u64,
    pub total_no_staked: u64,

    pub available_yes_fees: u64,
    pub available_no_fees: u64,

    pub creation_timestamp: i64,
    pub resolution_timestamp: i64,
    pub yes_last_update_ts: i64,
    pub no_last_update_ts: i64,

    pub fee_bps: u16,
    pub status: PoolStatus,
    pub vault_bump: u8,
    pub bump: u8,
}

impl Pool {
    pub const SIZE: usize = DISCRIMINATOR_SIZE + Pool::INIT_SPACE;

    pub fn accrue_yes(&mut self, now: i64) -> Result<()> {
        let dt = now
            .checked_sub(self.yes_last_update_ts)
            .ok_or(ReflexError::MathOverflow)?;

        require!(dt >= 0, ReflexError::Unauthorized);

        let delta = (self.total_yes_staked as u128)
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

        let delta = (self.total_no_staked as u128)
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
        self.total_yes_staked = self
            .total_yes_staked
            .checked_add(amount)
            .ok_or(ReflexError::MathOverflow)?;

        Ok(())
    }

    pub fn add_no_staked(&mut self, amount: u64) -> Result<()> {
        self.total_no_staked = self
            .total_no_staked
            .checked_add(amount)
            .ok_or(ReflexError::MathOverflow)?;

        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.status == PoolStatus::Open
    }

    pub fn is_yes(&self) -> bool {
        self.status == PoolStatus::Yes
    }

    pub fn is_no(&self) -> bool {
        self.status == PoolStatus::No
    }

    pub fn can_settle(&self) -> bool {
        self.is_open() || self.status == PoolStatus::Resolving
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, PartialEq)]
pub enum PoolStatus {
    Open,
    Yes,
    No,
    Resolving,
    Cancelled,
}
