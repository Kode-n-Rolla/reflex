use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};

use crate::{constants::*, error::ReflexError, Pool, Position};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(
        mut,
        seeds = [POOL_SEED, market.key().as_ref()],
        bump = pool.bump,
        constraint = pool.market == market.key() @ ReflexError::InvalidMarket
    )]
    pub pool: Box<Account<'info, Pool>>,

    /// CHECK: the market is a local mock account, so it is not owned by the World
    /// program. Accepting a caller-supplied `predict_program` would not authenticate
    /// it; binding the account to `pool.market` and validating its layout is safer here.
    #[account(constraint = market.data_len() == WORLD_MARKET_DATA_LEN)]
    pub market: UncheckedAccount<'info>,

    #[account(
        mut,
        close = staker,
        seeds = [POSITION_SEED, pool.key().as_ref(), staker.key().as_ref()],
        bump = position.bump
    )]
    pub position: Box<Account<'info, Position>>,

    #[account(address = pool.incentive_mint)]
    pub incentive_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(address = pool.yes_outcome_mint)]
    pub yes_outcome_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(address = pool.no_outcome_mint)]
    pub no_outcome_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = incentive_mint,
        token::authority = pool,
        seeds = [VAULT_SEED, pool.key().as_ref()],
        bump = pool.vault_bump
    )]
    pub pool_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = yes_outcome_mint,
        token::authority = pool,
        seeds = [VAULT_SEED, pool.key().as_ref(), yes_outcome_mint.key().as_ref()],
        bump
    )]
    pub yes_outcome_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = no_outcome_mint,
        token::authority = pool,
        seeds = [VAULT_SEED, pool.key().as_ref(), no_outcome_mint.key().as_ref()],
        bump
    )]
    pub no_outcome_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = staker,
        associated_token::mint = incentive_mint,
        associated_token::authority = staker
    )]
    pub staker_incentive_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = yes_outcome_mint,
        associated_token::authority = staker
    )]
    pub staker_yes_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = no_outcome_mint,
        associated_token::authority = staker
    )]
    pub staker_no_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl ClaimRewards<'_> {
    pub fn proccess_ix(&mut self) -> Result<()> {
        let resolution_timestamp = self.pool.resolution_timestamp;

        self.position.accrue_yes(resolution_timestamp)?;
        self.position.accrue_no(resolution_timestamp)?;

        let (winning_position_stake_seconds, winning_pool_stake_seconds) = if self.pool.is_yes() {
            (self.position.yes_stake_seconds, self.pool.yes_stake_seconds)
        } else if self.pool.is_no() {
            (self.position.no_stake_seconds, self.pool.no_stake_seconds)
        } else {
            return Err(ReflexError::PooltNotSettled.into());
        };

        let payout: u64 = if winning_pool_stake_seconds == 0 {
            0
        } else {
            winning_position_stake_seconds
                .checked_mul(self.pool.total_incentive_amount as u128)
                .ok_or(ReflexError::MathOverflow)?
                .checked_div(winning_pool_stake_seconds)
                .ok_or(ReflexError::MathOverflow)?
                .try_into()
                .map_err(|_| ReflexError::MathOverflow)?
        };

        require!(
            self.position.yes_staked > 0 || self.position.no_staked > 0 || payout > 0,
            ReflexError::ZeroRewards
        );

        let market = self.market.key();
        let signer_seeds: &[&[&[u8]]] = &[&[POOL_SEED, market.as_ref(), &[self.pool.bump]]];

        if self.position.yes_staked > 0 {
            let cpi_accounts = token_interface::TransferChecked {
                from: self.yes_outcome_vault.to_account_info(),
                mint: self.yes_outcome_mint.to_account_info(),
                to: self.staker_yes_ata.to_account_info(),
                authority: self.pool.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(*self.token_program.key, cpi_accounts).with_signer(signer_seeds);

            token_interface::transfer_checked(
                cpi_ctx,
                self.position.yes_staked,
                self.yes_outcome_mint.decimals,
            )?;
        }

        if self.position.no_staked > 0 {
            let cpi_accounts = token_interface::TransferChecked {
                from: self.no_outcome_vault.to_account_info(),
                mint: self.no_outcome_mint.to_account_info(),
                to: self.staker_no_ata.to_account_info(),
                authority: self.pool.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(*self.token_program.key, cpi_accounts).with_signer(signer_seeds);

            token_interface::transfer_checked(
                cpi_ctx,
                self.position.no_staked,
                self.no_outcome_mint.decimals,
            )?;
        }

        if payout > 0 {
            let cpi_accounts = token_interface::TransferChecked {
                from: self.pool_vault.to_account_info(),
                mint: self.incentive_mint.to_account_info(),
                to: self.staker_incentive_ata.to_account_info(),
                authority: self.pool.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(*self.token_program.key, cpi_accounts).with_signer(signer_seeds);

            token_interface::transfer_checked(cpi_ctx, payout, self.incentive_mint.decimals)?;
        }

        Ok(())
    }
}
