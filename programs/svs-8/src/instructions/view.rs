//! View instructions: read-only queries for vault state.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::set_return_data;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::MAX_ORACLE_STALENESS,
    error::VaultError,
    math::{self, convert_to_assets, convert_to_shares, Rounding},
    state::{AssetEntry, MultiAssetVault, OraclePrice},
};

#[derive(Accounts)]
pub struct VaultView<'info> {
    pub vault: Account<'info, MultiAssetVault>,

    #[account(constraint = shares_mint.key() == vault.shares_mint)]
    pub shares_mint: InterfaceAccount<'info, Mint>,

    // remaining_accounts: [AssetEntry, asset_vault, oracle] × num_assets
}

/// Compute total portfolio value from remaining_accounts
fn compute_portfolio_value(
    vault: &MultiAssetVault,
    remaining: &[AccountInfo],
) -> Result<u64> {
    let num_assets = vault.num_assets as usize;
    if num_assets == 0 {
        return Ok(0);
    }

    require!(
        remaining.len() >= num_assets * 3,
        VaultError::RemainingAccountsMismatch
    );

    let clock = Clock::get()?;
    let mut total: u128 = 0;

    for i in 0..num_assets {
        let entry_info = &remaining[i * 3];
        let vault_info = &remaining[i * 3 + 1];
        let oracle_info = &remaining[i * 3 + 2];

        require!(
            *entry_info.owner == crate::ID,
            VaultError::RemainingAccountsMismatch
        );
        let entry_data = entry_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &entry_data[..])
            .map_err(|_| error!(VaultError::RemainingAccountsMismatch))?;

        let vault_data = vault_info.try_borrow_data()?;
        require!(vault_data.len() >= 72, VaultError::RemainingAccountsMismatch);
        let balance = u64::from_le_bytes(
            vault_data[64..72]
                .try_into()
                .map_err(|_| error!(VaultError::MathOverflow))?,
        );

        require!(
            *oracle_info.owner == crate::ID,
            VaultError::InvalidOracle
        );
        let oracle_data = oracle_info.try_borrow_data()?;
        let oracle = OraclePrice::try_deserialize(&mut &oracle_data[..])
            .map_err(|_| error!(VaultError::InvalidOracle))?;
        require!(
            clock.unix_timestamp - oracle.updated_at <= MAX_ORACLE_STALENESS as i64,
            VaultError::OracleStale
        );

        let value = math::asset_value(balance, oracle.price, entry.asset_decimals)?;
        total = total
            .checked_add(value as u128)
            .ok_or(error!(VaultError::MathOverflow))?;

        drop(entry_data);
        drop(vault_data);
        drop(oracle_data);
    }

    u64::try_from(total).map_err(|_| error!(VaultError::MathOverflow))
}

/// Get total portfolio value in base units
pub fn total_portfolio_value(ctx: Context<VaultView>) -> Result<()> {
    let value = compute_portfolio_value(&ctx.accounts.vault, ctx.remaining_accounts)?;
    set_return_data(&value.to_le_bytes());
    Ok(())
}

/// Preview shares for a deposit of the given base-unit value
pub fn preview_deposit(ctx: Context<VaultView>, deposit_value: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let total_value = compute_portfolio_value(vault, ctx.remaining_accounts)?;

    let shares = convert_to_shares(
        deposit_value,
        total_value,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    set_return_data(&shares.to_le_bytes());
    Ok(())
}

/// Preview assets value for redeeming shares (floor rounding)
pub fn preview_redeem(ctx: Context<VaultView>, shares: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let total_value = compute_portfolio_value(vault, ctx.remaining_accounts)?;

    let assets = convert_to_assets(
        shares,
        total_value,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    set_return_data(&assets.to_le_bytes());
    Ok(())
}
