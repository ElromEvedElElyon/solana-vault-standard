//! Multi-asset vault state account definitions.

use anchor_lang::prelude::*;

use crate::constants::MULTI_VAULT_SEED;

/// Multi-asset basket vault. Holds N underlying SPL tokens with oracle-based pricing.
/// Seeds: ["multi_vault", vault_id.to_le_bytes()]
#[account]
pub struct MultiAssetVault {
    /// Vault admin who can pause/unpause, add/remove assets, rebalance
    pub authority: Pubkey,
    /// LP token mint (shares)
    pub shares_mint: Pubkey,
    /// Total shares outstanding (cached for compute efficiency)
    pub total_shares: u64,
    /// Virtual offset exponent for inflation attack protection
    pub decimals_offset: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Emergency pause flag
    pub paused: bool,
    /// Unique vault identifier
    pub vault_id: u64,
    /// Number of assets in basket (max 8)
    pub num_assets: u8,
    /// Base unit decimal precision (e.g., 6 for USD)
    pub base_decimals: u8,
    /// Reserved for future upgrades
    pub _reserved: [u8; 64],
}

impl MultiAssetVault {
    pub const LEN: usize = 8 +  // discriminator
        32 +  // authority
        32 +  // shares_mint
        8 +   // total_shares
        1 +   // decimals_offset
        1 +   // bump
        1 +   // paused
        8 +   // vault_id
        1 +   // num_assets
        1 +   // base_decimals
        64;   // _reserved

    pub const SEED_PREFIX: &'static [u8] = MULTI_VAULT_SEED;
}

/// Per-asset configuration within a multi-asset vault.
/// Seeds: ["asset_entry", vault_pda, asset_mint]
#[account]
pub struct AssetEntry {
    /// Parent vault
    pub vault: Pubkey,
    /// Asset token mint
    pub asset_mint: Pubkey,
    /// PDA-owned token account holding this asset
    pub asset_vault: Pubkey,
    /// Price oracle for this asset (Pyth, Switchboard, or svs-oracle)
    pub oracle: Pubkey,
    /// Target allocation in basis points (10000 = 100%)
    pub target_weight_bps: u16,
    /// Asset mint decimals (cached)
    pub asset_decimals: u8,
    /// Position in basket (0-indexed)
    pub index: u8,
    /// PDA bump seed
    pub bump: u8,
}

impl AssetEntry {
    pub const LEN: usize = 8 +  // discriminator
        32 +  // vault
        32 +  // asset_mint
        32 +  // asset_vault
        32 +  // oracle
        2 +   // target_weight_bps
        1 +   // asset_decimals
        1 +   // index
        1;    // bump
}

/// Simple oracle price account for testing and custom feeds.
/// In production, use Pyth or Switchboard directly.
/// Seeds: ["oracle_price", asset_mint]
#[account]
pub struct OraclePrice {
    /// Asset this price is for
    pub asset_mint: Pubkey,
    /// Price in base units (e.g., 1 SOL = 87_370_000 if base_decimals=6)
    pub price: u64,
    /// Last update unix timestamp
    pub updated_at: i64,
    /// Authority who can update this price
    pub authority: Pubkey,
    /// PDA bump seed
    pub bump: u8,
}

impl OraclePrice {
    pub const LEN: usize = 8 +  // discriminator
        32 +  // asset_mint
        8 +   // price
        8 +   // updated_at
        32 +  // authority
        1;    // bump
}
