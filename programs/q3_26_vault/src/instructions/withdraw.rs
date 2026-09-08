use crate::{
    constants::{STATE, VAULT_SEED},
    error::ErrorCode,
    state::VaultState,
};
use anchor_lang::{prelude::*, solana_program};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [STATE, user.key().as_ref()],
        bump = vault_state.state_bump
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [VAULT_SEED, user.key().as_ref()],
        bump = vault_state.vault_bump
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Withdraw<'info> {
    pub fn withdraw(&self, amount: u64) -> Result<()> {
        require!(amount > 0, ErrorCode::InvalidAmount);

        let vault_lamports = self.vault.lamports();
        require!(amount <= vault_lamports, ErrorCode::InsufficientBalance);

        let rent_exempt = Rent::get()?.minimum_balance(0);
        let withdrawable = vault_lamports
            .checked_sub(rent_exempt)
            .ok_or(ErrorCode::InsufficientBalance)?;
        require!(amount <= withdrawable, ErrorCode::InsufficientBalance);

        let vault_bump = self.vault_state.vault_bump;
        let user_key = self.user.key();
        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, user_key.as_ref(), &[vault_bump]]];

        let ix = solana_program::system_instruction::transfer(
            &self.vault.key(),
            &self.user.key(),
            amount,
        );

        solana_program::program::invoke_signed(
            &ix,
            &[
                self.vault.to_account_info(),
                self.user.to_account_info(),
                self.system_program.to_account_info(),
            ],
            signer_seeds,
        )?;

        Ok(())
    }
}
