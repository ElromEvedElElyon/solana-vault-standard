//! Preview deposit: read-only query returning shares for a given SOL amount.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::set_return_data;
use anchor_spl::token_interface::Mint;

use crate::{
    math::{convert_to_shares, Rounding},
    state::NativeSolVault,
};

#[derive(Accounts)]
pub struct PreviewDeposit<'info> {
    pub vault: Account<'info, NativeSolVault>,

    #[account(constraint = shares_mint.key() == vault.shares_mint)]
    pub shares_mint: InterfaceAccount<'info, Mint>,
}

/// Preview how many shares would be minted for given lamports (floor rounding).
pub fn handler(ctx: Context<PreviewDeposit>, lamports: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let total_assets = vault.total_sol;

    let shares = convert_to_shares(
        lamports,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    set_return_data(&shares.to_le_bytes());
    Ok(())
}
