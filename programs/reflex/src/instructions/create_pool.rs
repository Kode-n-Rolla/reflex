use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{constants::*, error::ReflexError, Config, Pool, PoolStatus};

#[derive(Accounts)]
#[instruction(incentive_amount: u64)]
pub struct CreatePool<'info> {
    #[account(
        mut,
        constraint = incentive_amount > 0 @ ReflexError::ZeroRewards
    )]
    pub creator: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    /// CHECK: validated as a local 320-byte market fixture containing both mints.
    #[account(
        constraint = market.data_len() == WORLD_MARKET_DATA_LEN
            @ ReflexError::InvalidMarket,
        constraint = market_has_outcome_mints(
            &market,
            &yes_outcome_mint.key(),
            &no_outcome_mint.key()
        ) @ ReflexError::WrongMarketMints
    )]
    pub market: UncheckedAccount<'info>,

    #[account(
        init,
        payer = creator,
        space = Pool::SIZE,
        seeds = [POOL_SEED, market.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mint::token_program = token_program,
        constraint = is_supported_incentive_mint(&incentive_mint, &creator.key())
            @ ReflexError::UnsupportedIncentiveMint
    )]
    pub incentive_mint: InterfaceAccount<'info, Mint>,
    #[account(mint::token_program = token_program)]
    pub yes_outcome_mint: InterfaceAccount<'info, Mint>,
    #[account(
        mint::token_program = token_program,
        constraint = no_outcome_mint.key() != yes_outcome_mint.key()
            @ ReflexError::WrongMarketMints
    )]
    pub no_outcome_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = incentive_mint,
        token::authority = creator,
        token::token_program = token_program
    )]
    pub creator_incentive_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        token::mint = incentive_mint,
        token::authority = pool,
        token::token_program = token_program,
        seeds = [VAULT_SEED, pool.key().as_ref()],
        bump
    )]
    pub pool_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        token::mint = yes_outcome_mint,
        token::authority = pool,
        token::token_program = token_program,
        seeds = [VAULT_SEED, pool.key().as_ref(), yes_outcome_mint.key().as_ref()],
        bump
    )]
    pub yes_outcome_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        token::mint = no_outcome_mint,
        token::authority = pool,
        token::token_program = token_program,
        seeds = [VAULT_SEED, pool.key().as_ref(), no_outcome_mint.key().as_ref()],
        bump
    )]
    pub no_outcome_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> CreatePool<'info> {
    pub fn proccess_ix(&mut self, incentive_amount: u64, bumps: &CreatePoolBumps) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;

        self.pool.set_inner(Pool {
            creator: self.creator.key(),
            market: self.market.key(),
            incentive_mint: self.incentive_mint.key(),
            yes_outcome_mint: self.yes_outcome_mint.key(),
            no_outcome_mint: self.no_outcome_mint.key(),
            yes_stake_seconds: 0,
            no_stake_seconds: 0,
            total_incentive_amount: incentive_amount,
            total_yes_staked: 0,
            total_no_staked: 0,
            available_yes_fees: 0,
            available_no_fees: 0,
            creation_timestamp: now,
            resolution_timestamp: 0,
            yes_last_update_ts: now,
            no_last_update_ts: now,
            fee_bps: self.config.fee_bps,
            briber_fee_bps: self.config.briber_fee_bps,
            status: PoolStatus::Open,
            vault_bump: bumps.pool_vault,
            bump: bumps.pool,
        });

        let cpi_accounts = token_interface::TransferChecked {
            from: self.creator_incentive_account.to_account_info(),
            mint: self.incentive_mint.to_account_info(),
            to: self.pool_vault.to_account_info(),
            authority: self.creator.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(*self.token_program.key, cpi_accounts);

        token_interface::transfer_checked(cpi_ctx, incentive_amount, self.incentive_mint.decimals)?;

        Ok(())
    }
}

fn is_supported_incentive_mint(mint: &InterfaceAccount<'_, Mint>, _creator: &Pubkey) -> bool {
    if mint.key() == USDC_MINT || mint.key() == WSOL_MINT {
        return true;
    }

    #[cfg(feature = "local-test")]
    {
        return mint.mint_authority
            == anchor_lang::solana_program::program_option::COption::Some(*_creator);
    }

    #[cfg(not(feature = "local-test"))]
    false
}

fn market_has_outcome_mints(
    market: &UncheckedAccount<'_>,
    yes_mint: &Pubkey,
    no_mint: &Pubkey,
) -> bool {
    let Ok(data) = market.try_borrow_data() else {
        return false;
    };

    if data.len() != WORLD_MARKET_DATA_LEN {
        return false;
    }

    data[WORLD_YES_MINT_OFFSET..WORLD_NO_MINT_OFFSET] == yes_mint.to_bytes()
        && data[WORLD_NO_MINT_OFFSET..WORLD_NO_MINT_OFFSET + 32] == no_mint.to_bytes()
}
