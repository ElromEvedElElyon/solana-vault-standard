//! Add an asset to the basket: creates AssetEntry PDA, OraclePrice PDA, and asset vault.

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    constants::{ASSET_ENTRY_SEED, MAX_BASKET_ASSETS, ORACLE_SEED, WEIGHT_BPS_DENOMINATOR},
    error::VaultError,
    events::AssetAdded,
    state::{AssetEntry, MultiAssetVault, OraclePrice},
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
        space = OraclePrice::LEN,
        seeds = [ORACLE_SEED, vault.key().as_ref(), asset_mint.key().as_ref()],
        bump,
    )]
    pub oracle_price: Account<'info, OraclePrice>,

    #[account(
        init,
        payer = authority,
        associated_token::mint = asset_mint,
        associated_token::authority = vault,
        associated_token::token_program = token_program,
    )]
    pub asset_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    // remaining_accounts: existing AssetEntry accounts for weight validation
}

pub fn handler(ctx: Context<AddAsset>, target_weight_bps: u16, initial_price: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;

    require!(
        vault.num_assets < MAX_BASKET_ASSETS,
        VaultError::MaxAssetsExceeded
    );

    // Sum existing weights from remaining_accounts
    let mut current_total_weight: u16 = 0;
    for account_info in ctx.remaining_accounts.iter() {
        require!(
            *account_info.owner == crate::ID,
            VaultError::RemainingAccountsMismatch
        );
        let data = account_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &data[..])
            .map_err(|_| error!(VaultError::RemainingAccountsMismatch))?;
        require!(entry.vault == vault.key(), VaultError::AssetNotFound);
        current_total_weight = current_total_weight
            .checked_add(entry.target_weight_bps)
            .ok_or(VaultError::MathOverflow)?;
    }

    // Validate new weight won't exceed 10000 bps
    require!(
        current_total_weight
            .checked_add(target_weight_bps)
            .ok_or(VaultError::MathOverflow)?
            <= WEIGHT_BPS_DENOMINATOR,
        VaultError::InvalidWeight
    );

    // Initialize OraclePrice
    let oracle_price = &mut ctx.accounts.oracle_price;
    oracle_price.vault = ctx.accounts.vault.key();
    oracle_price.asset_mint = ctx.accounts.asset_mint.key();
    oracle_price.price = initial_price;
    oracle_price.updated_at = Clock::get()?.unix_timestamp;
    oracle_price.bump = ctx.bumps.oracle_price;

    // Initialize AssetEntry
    let asset_entry = &mut ctx.accounts.asset_entry;
    asset_entry.vault = ctx.accounts.vault.key();
    asset_entry.asset_mint = ctx.accounts.asset_mint.key();
    asset_entry.asset_vault = ctx.accounts.asset_vault.key();
    asset_entry.oracle = ctx.accounts.oracle_price.key();
    asset_entry.target_weight_bps = target_weight_bps;
    asset_entry.asset_decimals = ctx.accounts.asset_mint.decimals;
    asset_entry.index = vault.num_assets;
    asset_entry.bump = ctx.bumps.asset_entry;

    // Increment asset count
    let vault = &mut ctx.accounts.vault;
    vault.num_assets = vault
        .num_assets
        .checked_add(1)
        .ok_or(VaultError::MathOverflow)?;

    emit!(AssetAdded {
        vault: vault.key(),
        asset_mint: ctx.accounts.asset_mint.key(),
        oracle: ctx.accounts.oracle_price.key(),
        target_weight_bps,
        index: asset_entry.index,
    });

    Ok(())
}
