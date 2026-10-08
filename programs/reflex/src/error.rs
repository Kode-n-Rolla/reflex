use anchor_lang::prelude::*;

#[error_code]
pub enum ReflexError {
    #[msg("Only the authority can call")]
    Unauthorized,
    #[msg("The selected mint is not correct")]
    WrongMint,
    #[msg("Math Overflow")]
    MathOverflow,
    #[msg("The market is not open")]
    MarketIsNotOpen,
    #[msg("The Pool can not be settled")]
    CanNotSettle,
    #[msg("The Pool is not settled")]
    PooltNotSettled,
    #[msg("The rewards must be greater than zero")]
    ZeroRewards,
}
