//! Vault error codes.

use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    #[msg("Vault is paused")]
    VaultPaused,

    #[msg("Asset decimals must be <= 9")]
    InvalidAssetDecimals,

    #[msg("Arithmetic overflow")]
    MathOverflow,

    #[msg("Division by zero")]
    DivisionByZero,

    #[msg("Insufficient shares balance")]
    InsufficientShares,

    #[msg("Insufficient assets in vault")]
    InsufficientAssets,

    #[msg("Unauthorized - caller is not vault authority")]
    Unauthorized,

    #[msg("Deposit amount below minimum threshold")]
    DepositTooSmall,

    #[msg("Vault is not paused")]
    VaultNotPaused,

    #[msg("Maximum basket assets exceeded")]
    MaxAssetsExceeded,

    #[msg("Asset weights must sum to 10000 bps")]
    InvalidWeight,

    #[msg("Asset already exists in basket")]
    AssetAlreadyExists,

    #[msg("Asset not found in basket")]
    AssetNotFound,

    #[msg("Asset vault has non-zero balance")]
    AssetVaultNotEmpty,

    #[msg("Oracle price is stale")]
    OracleStale,

    #[msg("Invalid oracle data")]
    InvalidOracle,

    #[msg("Remaining accounts mismatch")]
    RemainingAccountsMismatch,

    // Module errors
    #[msg("Deposit would exceed global vault cap")]
    GlobalCapExceeded,

    #[msg("Entry fee exceeds maximum")]
    EntryFeeExceedsMax,

    #[msg("Lock duration exceeds maximum")]
    LockDurationExceedsMax,

    #[msg("Invalid fee configuration")]
    InvalidFeeConfig,

    #[msg("Invalid cap configuration")]
    InvalidCapConfig,
}
