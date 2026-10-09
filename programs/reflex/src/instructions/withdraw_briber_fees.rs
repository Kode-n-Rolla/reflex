use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{constants::*, error::ReflexError, Pool};

#[derive(Accounts)]
pub struct WithdrawBriberFees<'info> {
    #[account(mut, address = pool.creator @ ReflexError::Unauthorized)]
    pub creator: Signer<'info>,

    #[account(mut, seeds = [POOL_SEED, pool.market.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,

    #[account(address = pool.yes_outcome_mint)]
    pub yes_outcome_mint: InterfaceAccount<'info, Mint>,
    #[account(address = pool.no_outcome_mint)]
    pub no_outcome_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = yes_outcome_mint,
        token::authority = pool,
        seeds = [VAULT_SEED, pool.key().as_ref(), yes_outcome_mint.key().as_ref()],
        bump
    )]
    pub yes_outcome_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = no_outcome_mint,
        token::authority = pool,
        seeds = [VAULT_SEED, pool.key().as_ref(), no_outcome_mint.key().as_ref()],
        bump
    )]
    pub no_outcome_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = yes_outcome_mint,
        associated_token::authority = creator
    )]
    pub creator_yes_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = no_outcome_mint,
        associated_token::authority = creator
    )]
    pub creator_no_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl WithdrawBriberFees<'_> {
    pub fn proccess_ix(&mut self) -> Result<()> {
        let yes_fees = self.pool.available_yes_fees;
        let no_fees = self.pool.available_no_fees;
        require!(yes_fees > 0 || no_fees > 0, ReflexError::NoFeesAvailable);

        let market = self.pool.market;
        let signer_seeds: &[&[&[u8]]] = &[&[POOL_SEED, market.as_ref(), &[self.pool.bump]]];

        if yes_fees > 0 {
            let accounts = token_interface::TransferChecked {
                from: self.yes_outcome_vault.to_account_info(),
                mint: self.yes_outcome_mint.to_account_info(),
                to: self.creator_yes_ata.to_account_info(),
                authority: self.pool.to_account_info(),
            };
            token_interface::transfer_checked(
                CpiContext::new(*self.token_program.key, accounts).with_signer(signer_seeds),
                yes_fees,
                self.yes_outcome_mint.decimals,
            )?;
        }

        if no_fees > 0 {
            let accounts = token_interface::TransferChecked {
                from: self.no_outcome_vault.to_account_info(),
                mint: self.no_outcome_mint.to_account_info(),
                to: self.creator_no_ata.to_account_info(),
                authority: self.pool.to_account_info(),
            };
            token_interface::transfer_checked(
                CpiContext::new(*self.token_program.key, accounts).with_signer(signer_seeds),
                no_fees,
                self.no_outcome_mint.decimals,
            )?;
        }

        self.pool.available_yes_fees = 0;
        self.pool.available_no_fees = 0;
        Ok(())
    }
}
