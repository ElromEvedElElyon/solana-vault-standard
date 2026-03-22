//! Program constants: PDA seeds, limits, and configuration.

pub const VAULT_SEED: &[u8] = b"multi_vault";
pub const SHARES_MINT_SEED: &[u8] = b"shares";
pub const ASSET_ENTRY_SEED: &[u8] = b"asset_entry";

pub const MAX_DECIMALS: u8 = 9;
pub const SHARES_DECIMALS: u8 = 9;

pub const MIN_DEPOSIT_AMOUNT: u64 = 1000;

/// Maximum assets in a basket
pub const MAX_BASKET_ASSETS: u8 = 8;

/// Weight basis points denominator (10000 = 100%)
pub const WEIGHT_BPS_DENOMINATOR: u16 = 10_000;

/// Maximum oracle staleness in seconds (5 minutes)
pub const MAX_ORACLE_STALENESS: u64 = 300;

/// Base decimals for portfolio valuation (6 = USD with 6 decimals)
pub const BASE_DECIMALS: u8 = 6;

/// PDA seed for oracle price accounts
pub const ORACLE_SEED: &[u8] = b"oracle";
