//! SVS-7: Native SOL Vault
//!
//! Tokenized vault that accepts native SOL deposits, auto-wraps them as wSOL,
//! and exposes a standard ERC-4626-style vault interface. On withdrawal, wSOL
//! is automatically unwrapped back to native SOL.
//!
//! Key features:
//! - Accept native SOL, auto-wrap to wSOL internally
//! - Standard vault interface (deposit, withdraw, preview, view)
//! - Shares represent proportional ownership of the SOL pool
//! - Auto-unwrap on withdrawal (user gets SOL, not wSOL)
//! - Inflation attack protection via virtual shares/assets offset
//! - Token-2022 shares mint
//! - All PDA bumps stored canonically

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

declare_id!("SVS7NativeSo1Vau1tXXXXXXXXXXXXXXXXXXXXXXXXXX");

#[program]
pub mod svs_7 {
    use super::*;

    /// Initialize a new native SOL vault
    pub fn initialize(
        ctx: Context<Initialize>,
        vault_id: u64,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        instructions::initialize::handler(ctx, vault_id, name, symbol, uri)
    }

    /// Deposit native SOL, receive vault shares
    /// SOL is auto-wrapped to wSOL internally
    /// Returns shares minted (floor rounding - favors vault)
    pub fn deposit_sol(
        ctx: Context<DepositSol>,
        lamports: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        instructions::deposit_sol::handler(ctx, lamports, min_shares_out)
    }

    /// Withdraw native SOL by burning vault shares
    /// wSOL is auto-unwrapped to SOL for the user
    /// Returns SOL (floor rounding - favors vault)
    pub fn withdraw_sol(
        ctx: Context<WithdrawSol>,
        shares: u64,
        min_sol_out: u64,
    ) -> Result<()> {
        instructions::withdraw_sol::handler(ctx, shares, min_sol_out)
    }

    /// Pause all vault operations (emergency)
    pub fn pause(ctx: Context<Admin>) -> Result<()> {
        instructions::admin::pause(ctx)
    }

    /// Unpause vault operations
    pub fn unpause(ctx: Context<Admin>) -> Result<()> {
        instructions::admin::unpause(ctx)
    }

    /// Transfer vault authority
    pub fn transfer_authority(ctx: Context<Admin>, new_authority: Pubkey) -> Result<()> {
        instructions::admin::transfer_authority(ctx, new_authority)
    }

    // ============ View Functions (CPI composable) ============

    /// Preview shares for a SOL deposit (floor rounding)
    pub fn preview_deposit(ctx: Context<PreviewDeposit>, lamports: u64) -> Result<()> {
        instructions::preview_deposit::handler(ctx, lamports)
    }

    /// Preview SOL for a share redemption (floor rounding)
    pub fn preview_withdraw(ctx: Context<PreviewWithdraw>, shares: u64) -> Result<()> {
        instructions::preview_withdraw::handler(ctx, shares)
    }

    /// Convert lamports to shares (floor rounding)
    pub fn convert_to_shares(ctx: Context<VaultView>, assets: u64) -> Result<()> {
        instructions::view::convert_to_shares_view(ctx, assets)
    }

    /// Convert shares to lamports (floor rounding)
    pub fn convert_to_assets(ctx: Context<VaultView>, shares: u64) -> Result<()> {
        instructions::view::convert_to_assets_view(ctx, shares)
    }

    /// Get total SOL in vault
    pub fn total_assets(ctx: Context<VaultView>) -> Result<()> {
        instructions::view::get_total_assets(ctx)
    }

    /// Max lamports depositable (u64::MAX or 0 if paused)
    pub fn max_deposit(ctx: Context<VaultView>) -> Result<()> {
        instructions::view::max_deposit(ctx)
    }

    /// Max shares mintable (u64::MAX or 0 if paused)
    pub fn max_mint(ctx: Context<VaultView>) -> Result<()> {
        instructions::view::max_mint(ctx)
    }

    /// Max lamports owner can withdraw
    pub fn max_withdraw(ctx: Context<VaultViewWithOwner>) -> Result<()> {
        instructions::view::max_withdraw(ctx)
    }

    /// Max shares owner can redeem
    pub fn max_redeem(ctx: Context<VaultViewWithOwner>) -> Result<()> {
        instructions::view::max_redeem(ctx)
    }
}
