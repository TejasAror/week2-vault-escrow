use crate::{
    constants::{STATE, VAULT_SEED},
    state::VaultState,
};
use anchor_lang::{prelude::*, solana_program};

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [STATE, user.key().as_ref()],
        bump = vault_state.state_bump,
        close = user
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

impl<'info> Close<'info> {
    pub fn close(&self) -> Result<()> {
        let vault_lamports = self.vault.lamports();

        if vault_lamports > 0 {
            let vault_bump = self.vault_state.vault_bump;
            let user_key = self.user.key();
            let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, user_key.as_ref(), &[vault_bump]]];

            let ix = solana_program::system_instruction::transfer(
                &self.vault.key(),
                &self.user.key(),
                vault_lamports,
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
        }

        Ok(())
    }
}
