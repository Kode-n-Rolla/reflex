use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{constants::*, error::ReflexError, Pool, Position};

#[derive(Accounts)]
pub struct DepositOutcome<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(
        mut,
        seeds = [POOL_SEED, market.key().as_ref()],
        bump = pool.bump,
        constraint = pool.is_open() @ ReflexError::MarketIsNotOpen,
        constraint = pool.market == market.key() @ ReflexError::InvalidMarket
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: the market is a local mock account, so it is not owned by the World
    /// program. Accepting a caller-supplied `predict_program` would not authenticate
    /// it; binding the account to `pool.market` and validating its layout is safer here.
    #[account(constraint = market.data_len() == WORLD_MARKET_DATA_LEN)]
    pub market: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = staker,
        space = Position::SIZE,
        seeds = [POSITION_SEED, pool.key().as_ref(), staker.key().as_ref()],
        bump
    )]
    pub position: Account<'info, Position>,

    pub outcome_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = outcome_mint,
        token::authority = pool,
        seeds = [VAULT_SEED, pool.key().as_ref(), outcome_mint.key().as_ref()],
        bump
    )]
    pub pool_outcome_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = outcome_mint,
        associated_token::authority = staker
    )]
    pub staker_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> DepositOutcome<'info> {
    pub fn proccess_ix(&mut self, amount: u64, bumps: &DepositOutcomeBumps) -> Result<()> {
        require!(
            self.outcome_mint.key() == self.pool.yes_outcome_mint
                || self.outcome_mint.key() == self.pool.no_outcome_mint,
            ReflexError::WrongMint
        );

        let cpi_accounts = token_interface::TransferChecked {
            authority: self.staker.to_account_info(),
            from: self.staker_ata.to_account_info(),
            mint: self.outcome_mint.to_account_info(),
            to: self.pool_outcome_vault.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(*self.token_program.key, cpi_accounts);

        token_interface::transfer_checked(cpi_ctx, amount, self.outcome_mint.decimals)?;

        if self.position.authority == Pubkey::default() {
            self.position.authority = self.staker.key();
            self.position.bump = bumps.position;
        }

        let now = Clock::get()?.unix_timestamp;

        if self.outcome_mint.key() == self.pool.yes_outcome_mint {
            self.pool.accrue_yes(now)?;
            self.position.accrue_yes(now)?;

            self.pool.add_yes_staked(amount)?;
            self.position.add_yes_staked(amount)?;
        } else if self.outcome_mint.key() == self.pool.no_outcome_mint {
            self.pool.accrue_no(now)?;
            self.position.accrue_no(now)?;

            self.pool.add_no_staked(amount)?;
            self.position.add_no_staked(amount)?;
        } else {
            return Err(ReflexError::WrongMint.into());
        }

        Ok(())
    }
}
