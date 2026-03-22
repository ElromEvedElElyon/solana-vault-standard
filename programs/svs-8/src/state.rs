//! Multi-asset vault state account definitions.

use anchor_lang::prelude::*;

use crate::constants::{ASSET_ENTRY_SEED, ORACLE_SEED, VAULT_SEED};

#[account]
pub struct MultiAssetVault {
    /// Vault admin who can pause/unpause, transfer authority, add/remove assets
    pub authority: Pubkey,
    /// LP token mint (shares) - Token-2022
    pub shares_mint: Pubkey,
    /// Current shares outstanding
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
    /// Decimal precision for weighted value (6 for USD)
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

    pub const SEED_PREFIX: &'static [u8] = VAULT_SEED;
}

#[account]
pub struct AssetEntry {
    /// Parent vault
    pub vault: Pubkey,
    /// Token mint for this asset
    pub asset_mint: Pubkey,
    /// PDA-owned token account holding this asset
    pub asset_vault: Pubkey,
    /// Price oracle account (stores price in base_decimals precision)
    pub oracle: Pubkey,
    /// Target allocation weight (10000 = 100%)
    pub target_weight_bps: u16,
    /// Asset mint decimals
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

    pub const SEED_PREFIX: &'static [u8] = ASSET_ENTRY_SEED;
}

#[account]
pub struct OraclePrice {
    pub vault: Pubkey,
    pub asset_mint: Pubkey,
    pub price: u64,
    pub updated_at: i64,
    pub bump: u8,
}

impl OraclePrice {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 8 + 1;
    pub const SEED_PREFIX: &'static [u8] = ORACLE_SEED;
}

// =============================================================================
// Access Mode (always available for IDL generation)
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessMode {
    #[default]
    Open,
    Whitelist,
    Blacklist,
}

// =============================================================================
// Module State Accounts (conditionally compiled with "modules" feature)
// =============================================================================

#[cfg(feature = "modules")]
pub mod module_state {
    use super::*;

    pub use svs_module_hooks::{
        ACCESS_CONFIG_SEED, CAP_CONFIG_SEED, FEE_CONFIG_SEED, FROZEN_ACCOUNT_SEED,
        LOCK_CONFIG_SEED, REWARD_CONFIG_SEED, SHARE_LOCK_SEED, USER_DEPOSIT_SEED, USER_REWARD_SEED,
    };

    #[account]
    pub struct FeeConfig {
        pub vault: Pubkey,
        pub fee_recipient: Pubkey,
        pub entry_fee_bps: u16,
        pub exit_fee_bps: u16,
        pub management_fee_bps: u16,
        pub performance_fee_bps: u16,
        pub high_water_mark: u64,
        pub last_fee_collection: i64,
        pub bump: u8,
    }

    impl FeeConfig {
        pub const LEN: usize = 8 + 32 + 32 + 2 + 2 + 2 + 2 + 8 + 8 + 1;
    }

    #[account]
    pub struct CapConfig {
        pub vault: Pubkey,
        pub global_cap: u64,
        pub per_user_cap: u64,
        pub bump: u8,
    }

    impl CapConfig {
        pub const LEN: usize = 8 + 32 + 8 + 8 + 1;
    }

    #[account]
    pub struct UserDeposit {
        pub vault: Pubkey,
        pub user: Pubkey,
        pub cumulative_assets: u64,
        pub bump: u8,
    }

    impl UserDeposit {
        pub const LEN: usize = 8 + 32 + 32 + 8 + 1;
    }

    #[account]
    pub struct LockConfig {
        pub vault: Pubkey,
        pub lock_duration: i64,
        pub bump: u8,
    }

    impl LockConfig {
        pub const LEN: usize = 8 + 32 + 8 + 1;
    }

    #[account]
    pub struct ShareLock {
        pub vault: Pubkey,
        pub owner: Pubkey,
        pub locked_until: i64,
        pub bump: u8,
    }

    impl ShareLock {
        pub const LEN: usize = 8 + 32 + 32 + 8 + 1;
    }

    #[account]
    pub struct AccessConfig {
        pub vault: Pubkey,
        pub mode: super::AccessMode,
        pub merkle_root: [u8; 32],
        pub bump: u8,
    }

    impl AccessConfig {
        pub const LEN: usize = 8 + 32 + 1 + 32 + 1;
    }

    #[account]
    pub struct FrozenAccount {
        pub vault: Pubkey,
        pub user: Pubkey,
        pub frozen_by: Pubkey,
        pub frozen_at: i64,
        pub bump: u8,
    }

    impl FrozenAccount {
        pub const LEN: usize = 8 + 32 + 32 + 32 + 8 + 1;
    }

    #[account]
    pub struct RewardConfig {
        pub vault: Pubkey,
        pub reward_mint: Pubkey,
        pub reward_vault: Pubkey,
        pub reward_authority: Pubkey,
        pub accumulated_per_share: u128,
        pub last_update: i64,
        pub bump: u8,
    }

    impl RewardConfig {
        pub const LEN: usize = 8 + 32 + 32 + 32 + 32 + 16 + 8 + 1;
    }

    #[account]
    pub struct UserReward {
        pub vault: Pubkey,
        pub user: Pubkey,
        pub reward_mint: Pubkey,
        pub reward_debt: u128,
        pub unclaimed: u64,
        pub bump: u8,
    }

    impl UserReward {
        pub const LEN: usize = 8 + 32 + 32 + 32 + 16 + 8 + 1;
    }
}

#[cfg(feature = "modules")]
pub use module_state::*;
