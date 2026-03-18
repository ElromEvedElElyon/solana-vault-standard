//! View instructions: read-only queries for vault state and conversions.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::set_return_data;
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::{
    math::{convert_to_assets, convert_to_shares, Rounding},
    state::NativeSolVault,
};

#[derive(Accounts)]
pub struct VaultView<'info> {
    pub vault: Account<'info, NativeSolVault>,

    #[account(constraint = shares_mint.key() == vault.shares_mint)]
    pub shares_mint: InterfaceAccount<'info, Mint>,
}

#[derive(Accounts)]
pub struct VaultViewWithOwner<'info> {
    pub vault: Account<'info, NativeSolVault>,

    #[account(constraint = shares_mint.key() == vault.shares_mint)]
    pub shares_mint: InterfaceAccount<'info, Mint>,

    #[account(
        constraint = owner_shares_account.mint == vault.shares_mint,
    )]
    pub owner_shares_account: InterfaceAccount<'info, TokenAccount>,
}

/// Convert assets (lamports) to shares (floor rounding)
pub fn convert_to_shares_view(ctx: Context<VaultView>, assets: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let total_assets = vault.total_sol;

    let shares = convert_to_shares(
        assets,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    set_return_data(&shares.to_le_bytes());
    Ok(())
}

/// Convert shares to assets (lamports, floor rounding)
pub fn convert_to_assets_view(ctx: Context<VaultView>, shares: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let total_assets = vault.total_sol;

    let assets = convert_to_assets(
        shares,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    set_return_data(&assets.to_le_bytes());
    Ok(())
}

/// Get total SOL managed by vault
pub fn get_total_assets(ctx: Context<VaultView>) -> Result<()> {
    set_return_data(&ctx.accounts.vault.total_sol.to_le_bytes());
    Ok(())
}

/// Max lamports depositable (u64::MAX if not paused, 0 if paused)
pub fn max_deposit(ctx: Context<VaultView>) -> Result<()> {
    let max = if ctx.accounts.vault.paused {
        0u64
    } else {
        u64::MAX
    };
    set_return_data(&max.to_le_bytes());
    Ok(())
}

/// Max shares mintable (u64::MAX if not paused, 0 if paused)
pub fn max_mint(ctx: Context<VaultView>) -> Result<()> {
    let max = if ctx.accounts.vault.paused {
        0u64
    } else {
        u64::MAX
    };
    set_return_data(&max.to_le_bytes());
    Ok(())
}

/// Max lamports owner can withdraw (limited by their shares)
pub fn max_withdraw(ctx: Context<VaultViewWithOwner>) -> Result<()> {
    if ctx.accounts.vault.paused {
        set_return_data(&0u64.to_le_bytes());
        return Ok(());
    }

    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let owner_shares = ctx.accounts.owner_shares_account.amount;
    let total_assets = vault.total_sol;

    let max_assets = convert_to_assets(
        owner_shares,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    let max = max_assets.min(total_assets);
    set_return_data(&max.to_le_bytes());
    Ok(())
}

/// Max shares owner can redeem (their share balance)
pub fn max_redeem(ctx: Context<VaultViewWithOwner>) -> Result<()> {
    let max = if ctx.accounts.vault.paused {
        0u64
    } else {
        ctx.accounts.owner_shares_account.amount
    };
    set_return_data(&max.to_le_bytes());
    Ok(())
}
