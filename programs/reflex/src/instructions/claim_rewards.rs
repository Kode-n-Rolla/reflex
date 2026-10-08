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
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: by owner and data len
    // #[account(/* owner = *predict_program.key, */ constraint = market.data_len() == 0)]
    // TODO:
    pub market: UncheckedAccount<'info>,

    #[account(
        mut,
        close = staker,
        seeds = [POSITION_SEED, pool.key().as_ref(), staker.key().as_ref()],
        bump = position.bump
    )]
    pub position: Account<'info, Position>,

    #[account(address = pool.incentive_mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    pub outcome_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = pool,
        seeds = [VAULT_SEED, pool.key().as_ref()],
        bump = pool.vault_bump
    )]
    pub pool_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = outcome_mint,
        token::authority = pool,
        seeds = [VAULT_SEED, pool.key().as_ref(), outcome_mint.key().as_ref()],
        bump
    )]
    pub pool_outcome_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = staker,
        associated_token::mint = mint,
        associated_token::authority = staker
    )]
    pub staker_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = outcome_mint,
        associated_token::authority = staker
    )]
    pub staker_outcome_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: by id
    // #[account(address = Pubkey::from_str(""))]
    pub predict_program: UncheckedAccount<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> ClaimRewards<'info> {
    // TODO: check fees
    pub fn proccess_ix(&mut self) -> Result<()> {
        require!(
            self.outcome_mint.key() == self.pool.yes_outcome_mint
                || self.outcome_mint.key() == self.pool.no_outcome_mint,
            ReflexError::WrongMint
        );

        let ts = self.pool.resolution_timestamp;

        // update pool and position
        let (stake_seconds, staked, s_w) = if self.pool.is_yes() {
            self.position.accrue_yes(ts)?;
            (
                self.position.yes_stake_seconds,
                self.position.yes_staked,
                self.pool.yes_stake_seconds,
            )
        } else if self.pool.is_no() {
            self.position.accrue_no(ts)?;
            (
                self.position.no_stake_seconds,
                self.position.no_staked,
                self.pool.no_stake_seconds,
            )
        } else {
            return Err(ReflexError::PooltNotSettled.into());
        };

        let payout: u64 = if s_w == 0 {
            0
        } else {
            (stake_seconds as u128)
                .checked_mul(self.pool.total_incentive_amount as u128)
                .ok_or(ReflexError::MathOverflow)?
                .checked_div(s_w)
                .ok_or(ReflexError::MathOverflow)?
                .try_into()
                .map_err(|_| ReflexError::MathOverflow)?
        };

        require!(staked > 0 || payout > 0, ReflexError::ZeroRewards);

        // transfer outcome winning to staker
        let market_binding = self.market.key();
        let signer_seeds: &[&[&[u8]]] = &[&[POOL_SEED, market_binding.as_ref(), &[self.pool.bump]]];

        if staked > 0 {
            let cpi_accounts = token_interface::TransferChecked {
                from: self.pool_outcome_vault.to_account_info(),
                mint: self.outcome_mint.to_account_info(),
                to: self.staker_outcome_ata.to_account_info(),
                authority: self.pool.to_account_info(),
            };

            let cpi_ctx =
                CpiContext::new(*self.token_program.key, cpi_accounts).with_signer(signer_seeds);

            token_interface::transfer_checked(cpi_ctx, staked, self.outcome_mint.decimals)?;
        }

        // transfer rewards to staker
        if payout > 0 {
            let cpi_accounts = token_interface::TransferChecked {
                from: self.pool_vault.to_account_info(),
                mint: self.mint.to_account_info(),
                to: self.staker_ata.to_account_info(),
                authority: self.pool.to_account_info(),
            };

            let cpi_ctx =
                CpiContext::new(*self.token_program.key, cpi_accounts).with_signer(signer_seeds);

            token_interface::transfer_checked(cpi_ctx, payout, self.mint.decimals)?;
        }
        
        Ok(())
    }
}
