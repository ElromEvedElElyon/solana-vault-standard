//! View instructions: read-only queries for vault state and conversions.
//! Uses streaming effective_total_assets for all share price calculations.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::set_return_data;
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::{
    math::{convert_to_assets, convert_to_shares, effective_total_assets, Rounding},
    state::ConfidentialStreamVault,
};

#[derive(Accounts)]
pub struct VaultView<'info> {
    pub vault: Account<'info, ConfidentialStreamVault>,

    #[account(constraint = shares_mint.key() == vault.shares_mint)]
    pub shares_mint: InterfaceAccount<'info, Mint>,

    #[account(constraint = asset_vault.key() == vault.asset_vault)]
    pub asset_vault: InterfaceAccount<'info, TokenAccount>,
}

pub fn preview_deposit(ctx: Context<VaultView>, assets: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;
    let total_assets = effective_total_assets(vault, clock.unix_timestamp)?;
    let total_shares = vault.total_shares;

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

pub fn preview_mint(ctx: Context<VaultView>, shares: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;
    let total_assets = effective_total_assets(vault, clock.unix_timestamp)?;
    let total_shares = vault.total_shares;

    let assets = convert_to_assets(
        shares,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Ceiling,
    )?;

    set_return_data(&assets.to_le_bytes());
    Ok(())
}

pub fn preview_withdraw(ctx: Context<VaultView>, assets: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;
    let total_assets = effective_total_assets(vault, clock.unix_timestamp)?;
    let total_shares = vault.total_shares;

    let shares = convert_to_shares(
        assets,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Ceiling,
    )?;

    set_return_data(&shares.to_le_bytes());
    Ok(())
}

pub fn preview_redeem(ctx: Context<VaultView>, shares: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;
    let total_assets = effective_total_assets(vault, clock.unix_timestamp)?;
    let total_shares = vault.total_shares;

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

pub fn convert_to_shares_view(ctx: Context<VaultView>, assets: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;
    let total_assets = effective_total_assets(vault, clock.unix_timestamp)?;
    let total_shares = vault.total_shares;

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

pub fn convert_to_assets_view(ctx: Context<VaultView>, shares: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;
    let total_assets = effective_total_assets(vault, clock.unix_timestamp)?;
    let total_shares = vault.total_shares;

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

/// Get effective total assets (base_assets + accrued streaming yield)
pub fn get_total_assets(ctx: Context<VaultView>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;
    let total = effective_total_assets(vault, clock.unix_timestamp)?;
    set_return_data(&total.to_le_bytes());
    Ok(())
}

pub fn max_deposit(ctx: Context<VaultView>) -> Result<()> {
    let max = if ctx.accounts.vault.paused {
        0u64
    } else {
        u64::MAX
    };
    set_return_data(&max.to_le_bytes());
    Ok(())
}

pub fn max_mint(ctx: Context<VaultView>) -> Result<()> {
    let max = if ctx.accounts.vault.paused {
        0u64
    } else {
        u64::MAX
    };
    set_return_data(&max.to_le_bytes());
    Ok(())
}

/// For confidential vaults, we cannot read encrypted balances on-chain.
/// Return vault's total assets as the upper bound.
pub fn max_withdraw(ctx: Context<VaultView>) -> Result<()> {
    let max = if ctx.accounts.vault.paused {
        0u64
    } else {
        let clock = Clock::get()?;
        effective_total_assets(&ctx.accounts.vault, clock.unix_timestamp)?
    };
    set_return_data(&max.to_le_bytes());
    Ok(())
}

/// For confidential vaults, we cannot read encrypted balances on-chain.
/// Return u64::MAX as a permissive upper bound.
pub fn max_redeem(ctx: Context<VaultView>) -> Result<()> {
    let max = if ctx.accounts.vault.paused {
        0u64
    } else {
        u64::MAX
    };
    set_return_data(&max.to_le_bytes());
    Ok(())
}
