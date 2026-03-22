//! SVS-8: Multi-Asset Basket Vault
//!
//! A single share mint represents proportional ownership of a basket of N underlying
//! SPL tokens. Deposits and redemptions can be made in any basket asset (single) or
//! all assets at once (proportional). Oracle prices determine share valuation.
//!
//! Key features:
//! - Up to 8 assets per basket
//! - Oracle-based portfolio valuation
//! - Single and proportional deposit/redeem
//! - Target weight allocation with authority rebalancing
//! - Inflation attack protection via virtual offset
//! - Slippage protection on all operations

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

declare_id!("SVS8MuLtiAssetVau1tXXXXXXXXXXXXXXXXXXXXXXXXX");

#[program]
pub mod svs_8 {
    use super::*;

    /// Initialize a new multi-asset basket vault
    pub fn initialize(
        ctx: Context<Initialize>,
        vault_id: u64,
        base_decimals: u8,
        name: String,
        symbol: String,
    ) -> Result<()> {
        instructions::initialize::handler(ctx, vault_id, base_decimals, name, symbol)
    }

    /// Add an asset to the basket
    pub fn add_asset(ctx: Context<AddAsset>, target_weight_bps: u16) -> Result<()> {
        instructions::add_asset::handler(ctx, target_weight_bps)
    }

    /// Remove an asset from the basket (must have zero balance)
    pub fn remove_asset(ctx: Context<RemoveAsset>) -> Result<()> {
        instructions::remove_asset::handler(ctx)
    }

    /// Update target weights for all assets (must sum to 10000)
    pub fn update_weights(ctx: Context<UpdateWeights>, new_weights: Vec<u16>) -> Result<()> {
        instructions::update_weights::handler(ctx, new_weights)
    }

    /// Deposit a single asset, receive shares based on oracle value
    pub fn deposit_single(
        ctx: Context<DepositSingle>,
        amount: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        instructions::deposit_single::handler(ctx, amount, min_shares_out)
    }

    /// Deposit all basket assets in target weight proportions
    pub fn deposit_proportional(
        ctx: Context<DepositProportional>,
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
    pub fn redeem_proportional(
        ctx: Context<RedeemProportional>,
        shares: u64,
        min_amounts_out: Vec<u64>,
    ) -> Result<()> {
        instructions::redeem_proportional::handler(ctx, shares, min_amounts_out)
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

    // ============ Oracle Admin (for testing) ============

    /// Initialize a price oracle for an asset
    pub fn initialize_oracle(ctx: Context<InitializeOracle>, initial_price: u64) -> Result<()> {
        instructions::oracle_admin::initialize_oracle(ctx, initial_price)
    }

    /// Update oracle price
    pub fn update_oracle(ctx: Context<UpdateOracle>, new_price: u64) -> Result<()> {
        instructions::oracle_admin::update_oracle(ctx, new_price)
    }
}
