//! SVS-8: Multi-Asset Basket Vault
//!
//! ERC-7575 adapted for Solana — a tokenized vault holding a basket of
//! multiple underlying SPL tokens. A single shares mint represents
//! proportional ownership of the entire portfolio. Deposits and
//! redemptions can be made in any accepted asset or proportionally.

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

declare_id!("9KNtodSV6CWpLH6tdJUbpotXZCCgSzFDJnr4KoE8mKDW");

#[program]
pub mod svs_8 {
    use super::*;

    /// Initialize a new multi-asset basket vault
    pub fn initialize(ctx: Context<Initialize>, vault_id: u64, base_decimals: u8) -> Result<()> {
        instructions::initialize::handler(ctx, vault_id, base_decimals)
    }

    /// Add an asset to the basket with target weight and initial oracle price
    pub fn add_asset(
        ctx: Context<AddAsset>,
        target_weight_bps: u16,
        initial_price: u64,
    ) -> Result<()> {
        instructions::add_asset::handler(ctx, target_weight_bps, initial_price)
    }

    /// Remove an asset from the basket (must have zero balance)
    pub fn remove_asset(ctx: Context<RemoveAsset>) -> Result<()> {
        instructions::remove_asset::handler(ctx)
    }

    /// Update target weights for all assets (must sum to 10000 bps)
    pub fn update_weights(ctx: Context<UpdateWeights>, new_weights: Vec<u16>) -> Result<()> {
        instructions::update_weights::handler(ctx, new_weights)
    }

    /// Set oracle price for an asset (authority only, devnet)
    pub fn set_price(ctx: Context<SetPrice>, price: u64) -> Result<()> {
        instructions::set_price::handler(ctx, price)
    }

    /// Deposit a single asset and receive shares based on its value
    pub fn deposit_single(
        ctx: Context<DepositSingle>,
        amount: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        instructions::deposit_single::handler(ctx, amount, min_shares_out)
    }

    /// Deposit all assets proportionally and receive shares
    pub fn deposit_proportional<'a>(
        ctx: Context<'_, '_, 'a, 'a, DepositProportional<'a>>,
        base_amount: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        instructions::deposit_proportional::handler(ctx, base_amount, min_shares_out)
    }

    /// Redeem shares for a single asset
    pub fn redeem_single(
        ctx: Context<RedeemSingle>,
        shares: u64,
        min_assets_out: u64,
    ) -> Result<()> {
        instructions::redeem_single::handler(ctx, shares, min_assets_out)
    }

    /// Redeem shares for proportional amounts of all basket assets
    pub fn redeem_proportional<'a>(
        ctx: Context<'_, '_, 'a, 'a, RedeemProportional<'a>>,
        shares: u64,
    ) -> Result<()> {
        instructions::redeem_proportional::handler(ctx, shares)
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

    // ============ View Functions ============

    /// Get total portfolio value in base units
    pub fn total_portfolio_value(ctx: Context<VaultView>) -> Result<()> {
        instructions::view::total_portfolio_value(ctx)
    }

    /// Preview shares for a single-asset deposit
    pub fn preview_deposit(ctx: Context<VaultView>, deposit_value: u64) -> Result<()> {
        instructions::view::preview_deposit(ctx, deposit_value)
    }

    /// Preview assets received for redeeming shares
    pub fn preview_redeem(ctx: Context<VaultView>, shares: u64) -> Result<()> {
        instructions::view::preview_redeem(ctx, shares)
    }
}
