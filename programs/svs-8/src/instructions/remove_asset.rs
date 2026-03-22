//! Remove an asset from the basket. Asset vault must have zero balance.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    close_account, CloseAccount, TokenAccount, TokenInterface,
};

use crate::{
    constants::{ASSET_ENTRY_SEED, ORACLE_SEED, VAULT_SEED},
    error::VaultError,
    events::AssetRemoved,
    state::{AssetEntry, MultiAssetVault, OraclePrice},
};

#[derive(Accounts)]
pub struct RemoveAsset<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority @ VaultError::Unauthorized,
    )]
    pub vault: Account<'info, MultiAssetVault>,

    #[account(
        mut,
        close = authority,
        seeds = [ASSET_ENTRY_SEED, vault.key().as_ref(), asset_entry.asset_mint.as_ref()],
        bump = asset_entry.bump,
        constraint = asset_entry.vault == vault.key() @ VaultError::AssetNotFound,
    )]
    pub asset_entry: Account<'info, AssetEntry>,

    #[account(
        mut,
        close = authority,
        seeds = [ORACLE_SEED, vault.key().as_ref(), asset_entry.asset_mint.as_ref()],
        bump = oracle_price.bump,
        constraint = oracle_price.vault == vault.key() @ VaultError::InvalidOracle,
    )]
    pub oracle_price: Account<'info, OraclePrice>,

    #[account(
        mut,
        constraint = asset_vault.key() == asset_entry.asset_vault @ VaultError::RemainingAccountsMismatch,
        constraint = asset_vault.amount == 0 @ VaultError::AssetVaultNotEmpty,
    )]
    pub asset_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<RemoveAsset>) -> Result<()> {
    let index = ctx.accounts.asset_entry.index;
    let asset_mint = ctx.accounts.asset_entry.asset_mint;
    let vault = &mut ctx.accounts.vault;

    // Close the token account (send lamports to authority)
    let vault_id_bytes = vault.vault_id.to_le_bytes();
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        &vault_id_bytes,
        &[vault.bump],
    ]];

    close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.asset_vault.to_account_info(),
            destination: ctx.accounts.authority.to_account_info(),
            authority: vault.to_account_info(),
        },
        signer_seeds,
    ))?;

    vault.num_assets = vault
        .num_assets
        .checked_sub(1)
        .ok_or(VaultError::MathOverflow)?;

    emit!(AssetRemoved {
        vault: vault.key(),
        asset_mint,
        index,
    });

    Ok(())
}
