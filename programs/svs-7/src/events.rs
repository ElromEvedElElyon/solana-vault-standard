//! Vault events emitted on state changes.

use anchor_lang::prelude::*;

#[event]
pub struct VaultInitialized {
    pub vault: Pubkey,
    pub authority: Pubkey,
    pub wsol_mint: Pubkey,
    pub shares_mint: Pubkey,
    pub wsol_vault: Pubkey,
    pub vault_id: u64,
}

#[event]
pub struct SolDeposited {
    pub vault: Pubkey,
    pub depositor: Pubkey,
    pub sol_amount: u64,
    pub shares_minted: u64,
    pub total_sol_after: u64,
}

#[event]
pub struct SolWithdrawn {
    pub vault: Pubkey,
    pub withdrawer: Pubkey,
    pub sol_amount: u64,
    pub shares_burned: u64,
    pub total_sol_after: u64,
}

#[event]
pub struct VaultStatusChanged {
    pub vault: Pubkey,
    pub paused: bool,
}

#[event]
pub struct AuthorityTransferred {
    pub vault: Pubkey,
    pub previous_authority: Pubkey,
    pub new_authority: Pubkey,
}
