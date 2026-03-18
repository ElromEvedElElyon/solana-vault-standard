//! Preview withdraw: read-only query returning SOL amount for given shares.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::set_return_data;
use anchor_spl::token_interface::Mint;

use crate::{
    math::{convert_to_assets, Rounding},
    state::NativeSolVault,
};

#[derive(Accounts)]
pub struct PreviewWithdraw<'info> {
    pub vault: Account<'info, NativeSolVault>,

    #[account(constraint = shares_mint.key() == vault.shares_mint)]
    pub shares_mint: InterfaceAccount<'info, Mint>,
}

/// Preview how many lamports would be received for given shares (floor rounding).
pub fn handler(ctx: Context<PreviewWithdraw>, shares: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let total_assets = vault.total_sol;

    let sol_amount = convert_to_assets(
        shares,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    set_return_data(&sol_amount.to_le_bytes());
    Ok(())
}
