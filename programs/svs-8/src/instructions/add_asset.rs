//! Add asset instruction: register a new asset in the vault basket.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    constants::{ASSET_ENTRY_SEED, BPS_DENOMINATOR, MAX_ASSETS, MULTI_VAULT_SEED},
    error::VaultError,
    events::AssetAdded,
    math::read_oracle_price,
    state::{AssetEntry, MultiAssetVault},
};

#[derive(Accounts)]
pub struct AddAsset<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority @ VaultError::Unauthorized,
    )]
    pub vault: Account<'info, MultiAssetVault>,

    pub asset_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: Validated via oracle reading in handler
    pub oracle: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        space = AssetEntry::LEN,
        seeds = [ASSET_ENTRY_SEED, vault.key().as_ref(), asset_mint.key().as_ref()],
        bump,
    )]
    pub asset_entry: Account<'info, AssetEntry>,

    #[account(
        init,
        payer = authority,
        token::mint = asset_mint,
        token::authority = vault,
        token::token_program = token_program,
    )]
    pub asset_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    // remaining_accounts: existing AssetEntry accounts for weight calculation
}

pub fn handler(ctx: Context<AddAsset>, target_weight_bps: u16) -> Result<()> {
    let vault = &mut ctx.accounts.vault;

    // Validate max assets not exceeded
    require!(vault.num_assets < MAX_ASSETS, VaultError::MaxAssetsExceeded);

    // Validate weight is positive
    require!(target_weight_bps > 0, VaultError::InvalidWeight);

    // Calculate current total weight from existing asset entries
    let mut current_total_weight: u16 = 0;
    for asset_info in ctx.remaining_accounts.iter() {
        let data = asset_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &data[..])?;
        require!(entry.vault == vault.key(), VaultError::AssetNotFound);
        current_total_weight = current_total_weight
            .checked_add(entry.target_weight_bps)
            .ok_or(VaultError::MathOverflow)?;
    }

    // Validate new weight won't exceed 10000 bps
    let new_total = current_total_weight
        .checked_add(target_weight_bps)
        .ok_or(VaultError::MathOverflow)?;
    require!(new_total <= BPS_DENOMINATOR, VaultError::InvalidWeight);

    // Validate oracle is readable (proves it works)
    read_oracle_price(
        &ctx.accounts.oracle.to_account_info(),
        &ctx.accounts.asset_mint.key(),
        600, // 10 minute staleness for setup
        ctx.program_id,
    )?;

    // Initialize AssetEntry PDA
    let asset_entry = &mut ctx.accounts.asset_entry;
    asset_entry.vault = vault.key();
    asset_entry.asset_mint = ctx.accounts.asset_mint.key();
    asset_entry.asset_vault = ctx.accounts.asset_vault.key();
    asset_entry.oracle = ctx.accounts.oracle.key();
    asset_entry.target_weight_bps = target_weight_bps;
    asset_entry.asset_decimals = ctx.accounts.asset_mint.decimals;
    asset_entry.index = vault.num_assets;
    asset_entry.bump = ctx.bumps.asset_entry;

    // Increment asset count
    vault.num_assets = vault.num_assets
        .checked_add(1)
        .ok_or(VaultError::MathOverflow)?;

    emit!(AssetAdded {
        vault: vault.key(),
        asset_mint: ctx.accounts.asset_mint.key(),
        oracle: ctx.accounts.oracle.key(),
        target_weight_bps,
        index: asset_entry.index,
    });

    Ok(())
}
