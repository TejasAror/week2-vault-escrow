use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,

    #[msg("Unauthorized access to the escrow")]
    Unauthorized,

    #[msg("Token balance is insufficient")]
    InsufficientBalance,

    #[msg("Invalid token mint")]
    InvalidMint,

    #[msg("Escrow has already been closed")]
    EscrowAlreadyClosed,

    #[msg("New escrow terms are not different")]
    InvalidTerms,
}
