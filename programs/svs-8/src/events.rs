//! Vault events emitted on state changes.

use anchor_lang::prelude::*;

#[event]
pub struct VaultInitialized {
    pub vault: Pubkey,
    pub authority: Pubkey,
    pub shares_mint: Pubkey,
    pub vault_id: u64,
}

#[event]
pub struct AssetAdded {
    pub vault: Pubkey,
    pub asset_mint: Pubkey,
    pub oracle: Pubkey,
    pub target_weight_bps: u16,
    pub index: u8,
}

#[event]
pub struct AssetRemoved {
    pub vault: Pubkey,
    pub asset_mint: Pubkey,
    pub index: u8,
}

#[event]
pub struct WeightsUpdated {
    pub vault: Pubkey,
}

#[event]
pub struct Deposit {
    pub vault: Pubkey,
    pub caller: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub assets: u64,
    pub shares: u64,
    pub value: u64,
}

#[event]
pub struct Withdraw {
    pub vault: Pubkey,
    pub caller: Pubkey,
    pub receiver: Pubkey,
    pub owner: Pubkey,
    pub assets: u64,
    pub shares: u64,
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

#[event]
pub struct PriceUpdated {
    pub vault: Pubkey,
    pub asset_mint: Pubkey,
    pub price: u64,
}
