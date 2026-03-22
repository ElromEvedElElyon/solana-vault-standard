//! Program constants: PDA seeds, limits, and decimals configuration.

pub const MULTI_VAULT_SEED: &[u8] = b"multi_vault";
pub const ASSET_ENTRY_SEED: &[u8] = b"asset_entry";
pub const SHARES_MINT_SEED: &[u8] = b"shares";

pub const MAX_DECIMALS: u8 = 9;
pub const SHARES_DECIMALS: u8 = 9;

pub const MIN_DEPOSIT_AMOUNT: u64 = 1000;

/// Maximum number of assets in a basket
pub const MAX_ASSETS: u8 = 8;

/// Basis points denominator (100% = 10000 bps)
pub const BPS_DENOMINATOR: u16 = 10_000;

/// Maximum oracle staleness in seconds (5 minutes)
pub const MAX_ORACLE_STALENESS: u64 = 300;

/// Maximum oracle confidence percentage in bps (1% = 100)
pub const MAX_ORACLE_CONFIDENCE_BPS: u64 = 100;
