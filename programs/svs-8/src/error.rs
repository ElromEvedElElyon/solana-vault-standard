//! Multi-asset vault error codes.

use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    #[msg("Vault is paused")]
    VaultPaused,

    #[msg("Vault is not paused")]
    VaultNotPaused,

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

    #[msg("Maximum number of assets exceeded (max 8)")]
    MaxAssetsExceeded,

    #[msg("Target weights must sum to 10000 basis points")]
    InvalidWeight,

    #[msg("Asset already exists in basket")]
    AssetAlreadyExists,

    #[msg("Asset not found in basket")]
    AssetNotFound,

    #[msg("Asset vault balance must be zero before removal")]
    AssetBalanceNotZero,

    #[msg("Oracle price is stale")]
    OracleStale,

    #[msg("Oracle confidence interval too wide")]
    OracleUncertain,

    #[msg("Invalid oracle account")]
    InvalidOracle,

    #[msg("Invalid program ID for CPI")]
    InvalidProgram,

    #[msg("Invalid remaining accounts count")]
    InvalidRemainingAccounts,

    #[msg("Asset mint mismatch")]
    AssetMintMismatch,

    #[msg("No assets in vault")]
    NoAssets,
}
