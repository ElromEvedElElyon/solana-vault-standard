//! Redeem shares for a single asset from the basket.

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{self, Burn, Token2022},
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::{ASSET_ENTRY_SEED, MAX_ORACLE_STALENESS, VAULT_SEED},
    error::VaultError,
    events::Withdraw as WithdrawEvent,
    math::{self, convert_to_assets, Rounding},
    state::{AssetEntry, MultiAssetVault, OraclePrice},
};

#[derive(Accounts)]
pub struct RedeemSingle<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Box<Account<'info, MultiAssetVault>>,

    #[account(
        mut,
        constraint = shares_mint.key() == vault.shares_mint,
    )]
    pub shares_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        constraint = user_shares_account.mint == vault.shares_mint,
        constraint = user_shares_account.owner == user.key(),
    )]
    pub user_shares_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        seeds = [ASSET_ENTRY_SEED, vault.key().as_ref(), asset_mint.key().as_ref()],
        bump = asset_entry.bump,
        constraint = asset_entry.vault == vault.key() @ VaultError::AssetNotFound,
    )]
    pub asset_entry: Box<Account<'info, AssetEntry>>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        constraint = user_asset_account.mint == asset_mint.key(),
        constraint = user_asset_account.owner == user.key(),
    )]
    pub user_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = asset_vault.key() == asset_entry.asset_vault,
    )]
    pub asset_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub asset_token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,

    // remaining_accounts: [AssetEntry, asset_vault, oracle] × num_assets
}

pub fn handler(ctx: Context<RedeemSingle>, shares: u64, min_assets_out: u64) -> Result<()> {
    require!(shares > 0, VaultError::ZeroAmount);
    require!(
        ctx.accounts.user_shares_account.amount >= shares,
        VaultError::InsufficientShares
    );

    let vault = &ctx.accounts.vault;
    let num_assets = vault.num_assets as usize;
    let remaining = &ctx.remaining_accounts;

    require!(
        remaining.len() >= num_assets * 3,
        VaultError::RemainingAccountsMismatch
    );

    let clock = Clock::get()?;
    let deposit_mint = ctx.accounts.asset_mint.key();

    // Compute total portfolio value
    let mut total_value: u128 = 0;
    let mut redeem_asset_price: u64 = 0;

    for i in 0..num_assets {
        let entry_info = &remaining[i * 3];
        let vault_info = &remaining[i * 3 + 1];
        let oracle_info = &remaining[i * 3 + 2];

        require!(
            *entry_info.owner == crate::ID,
            VaultError::RemainingAccountsMismatch
        );
        let entry_data = entry_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &entry_data[..])
            .map_err(|_| error!(VaultError::RemainingAccountsMismatch))?;
        require!(entry.vault == vault.key(), VaultError::AssetNotFound);

        let vault_data = vault_info.try_borrow_data()?;
        require!(vault_data.len() >= 72, VaultError::RemainingAccountsMismatch);
        let balance = u64::from_le_bytes(
            vault_data[64..72]
                .try_into()
                .map_err(|_| error!(VaultError::MathOverflow))?,
        );

        require!(oracle_info.key() == entry.oracle, VaultError::InvalidOracle);
        require!(
            *oracle_info.owner == crate::ID,
            VaultError::InvalidOracle
        );
        let oracle_data = oracle_info.try_borrow_data()?;
        let oracle = OraclePrice::try_deserialize(&mut &oracle_data[..])
            .map_err(|_| error!(VaultError::InvalidOracle))?;
        require!(
            clock.unix_timestamp - oracle.updated_at <= MAX_ORACLE_STALENESS as i64,
            VaultError::OracleStale
        );

        let value = math::asset_value(balance, oracle.price, entry.asset_decimals)?;
        total_value = total_value
            .checked_add(value as u128)
            .ok_or(error!(VaultError::MathOverflow))?;

        if entry.asset_mint == deposit_mint {
            redeem_asset_price = oracle.price;
        }

        drop(entry_data);
        drop(vault_data);
        drop(oracle_data);
    }

    require!(redeem_asset_price > 0, VaultError::AssetNotFound);

    let total_value =
        u64::try_from(total_value).map_err(|_| error!(VaultError::MathOverflow))?;

    // Convert shares to base-unit value (floor rounding - favors vault)
    let total_shares = ctx.accounts.shares_mint.supply;
    let redeem_value = convert_to_assets(
        shares,
        total_value,
        total_shares,
        ctx.accounts.vault.decimals_offset,
        Rounding::Floor,
    )?;

    // Convert base-unit value to asset amount
    // assets = redeem_value * 10^asset_decimals / price
    let asset_decimals = ctx.accounts.asset_entry.asset_decimals;
    let assets = (redeem_value as u128)
        .checked_mul(10u128.pow(asset_decimals as u32))
        .ok_or(error!(VaultError::MathOverflow))?
        .checked_div(redeem_asset_price as u128)
        .ok_or(error!(VaultError::DivisionByZero))?;
    let assets = u64::try_from(assets).map_err(|_| error!(VaultError::MathOverflow))?;

    require!(assets >= min_assets_out, VaultError::SlippageExceeded);
    require!(
        assets <= ctx.accounts.asset_vault.amount,
        VaultError::InsufficientAssets
    );

    // Burn shares
    token_2022::burn(
        CpiContext::new(
            ctx.accounts.token_2022_program.to_account_info(),
            Burn {
                mint: ctx.accounts.shares_mint.to_account_info(),
                from: ctx.accounts.user_shares_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        shares,
    )?;

    // Transfer asset from vault to user
    let vault_id_bytes = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = ctx.accounts.vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, &vault_id_bytes, &[bump]]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.asset_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.asset_vault.to_account_info(),
                to: ctx.accounts.user_asset_account.to_account_info(),
                mint: ctx.accounts.asset_mint.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        assets,
        ctx.accounts.asset_mint.decimals,
    )?;

    // Update vault total_shares
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_sub(shares)
        .ok_or(VaultError::MathOverflow)?;

    emit!(WithdrawEvent {
        vault: vault.key(),
        caller: ctx.accounts.user.key(),
        receiver: ctx.accounts.user.key(),
        owner: ctx.accounts.user.key(),
        assets,
        shares,
    });

    Ok(())
}
