//! Program constants: PDA seeds, limits, and decimals configuration.

pub const VAULT_SEED: &[u8] = b"native_sol_vault";
pub const SHARES_MINT_SEED: &[u8] = b"shares";
pub const WSOL_VAULT_SEED: &[u8] = b"wsol_vault";

/// Native SOL has 9 decimals
pub const SOL_DECIMALS: u8 = 9;

/// Shares use 9 decimals (same as SOL for 1:1 initial peg)
pub const SHARES_DECIMALS: u8 = 9;

/// Minimum deposit in lamports (prevent dust deposits)
pub const MIN_DEPOSIT_LAMPORTS: u64 = 1_000;
