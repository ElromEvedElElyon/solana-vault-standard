//! Vault state account definition.

use anchor_lang::prelude::*;

use crate::constants::VAULT_SEED;

#[account]
pub struct NativeSolVault {
    /// Vault admin who can pause/unpause and transfer authority
    pub authority: Pubkey,
    /// wSOL mint (always native mint)
    pub wsol_mint: Pubkey,
    /// LP token mint (shares) - Token-2022
    pub shares_mint: Pubkey,
    /// wSOL token account holding wrapped deposits
    pub wsol_vault: Pubkey,
    /// Virtual offset exponent for inflation attack protection
    /// SOL has 9 decimals, offset = 0 (9 - 9)
    pub decimals_offset: u8,
    /// PDA bump seed for vault
    pub bump: u8,
    /// PDA bump seed for shares mint
    pub shares_mint_bump: u8,
    /// PDA bump seed for wsol vault
    pub wsol_vault_bump: u8,
    /// Emergency pause flag
    pub paused: bool,
    /// Unique vault identifier (allows multiple native SOL vaults)
    pub vault_id: u64,
    /// Total SOL deposited (tracks wSOL balance for accounting)
    pub total_sol: u64,
    /// Reserved for future upgrades
    pub _reserved: [u8; 64],
}

impl NativeSolVault {
    pub const LEN: usize = 8 +   // discriminator
        32 +  // authority
        32 +  // wsol_mint
        32 +  // shares_mint
        32 +  // wsol_vault
        1 +   // decimals_offset
        1 +   // bump
        1 +   // shares_mint_bump
        1 +   // wsol_vault_bump
        1 +   // paused
        8 +   // vault_id
        8 +   // total_sol
        64;   // _reserved

    pub const SEED_PREFIX: &'static [u8] = VAULT_SEED;
}
