use crate::{constants::ESCROW_SEED, error::ErrorCode, state::Escrow};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(
        mut,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
        has_one = maker @ ErrorCode::Unauthorized,
    )]
    pub escrow: Box<Account<'info, Escrow>>,
}

impl<'info> Update<'info> {
    pub fn update(&mut self, receive: u64) -> Result<()> {
        require!(receive > 0, ErrorCode::InvalidAmount);
        require!(receive != self.escrow.receive, ErrorCode::InvalidTerms);

        self.escrow.receive = receive;

        Ok(())
    }
}
