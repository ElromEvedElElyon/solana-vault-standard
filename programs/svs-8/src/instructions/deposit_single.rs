//! Deposit a single asset into the multi-asset vault, receive shares.

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{self, MintTo, Token2022},
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::{ASSET_ENTRY_SEED, MAX_ORACLE_STALENESS, MIN_DEPOSIT_AMOUNT, VAULT_SEED},
    error::VaultError,
    events::Deposit as DepositEvent,
    math::{self, convert_to_shares, Rounding},
    state::{AssetEntry, MultiAssetVault, OraclePrice},
};

#[derive(Accounts)]
pub struct DepositSingle<'info> {
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

    // The specific asset being deposited
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

pub fn handler(ctx: Context<DepositSingle>, amount: u64, min_shares_out: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(amount >= MIN_DEPOSIT_AMOUNT, VaultError::DepositTooSmall);

    let vault = &ctx.accounts.vault;
    let num_assets = vault.num_assets as usize;
    let remaining = &ctx.remaining_accounts;

    require!(
        remaining.len() >= num_assets * 3,
        VaultError::RemainingAccountsMismatch
    );

    let clock = Clock::get()?;

    // Compute total portfolio value and find deposited asset's price
    let mut total_value: u128 = 0;
    let mut deposit_price: u64 = 0;
    let deposit_mint = ctx.accounts.asset_mint.key();

    for i in 0..num_assets {
        let entry_info = &remaining[i * 3];
        let vault_info = &remaining[i * 3 + 1];
        let oracle_info = &remaining[i * 3 + 2];

        // Read AssetEntry
        require!(
            *entry_info.owner == crate::ID,
            VaultError::RemainingAccountsMismatch
        );
        let entry_data = entry_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &entry_data[..])
            .map_err(|_| error!(VaultError::RemainingAccountsMismatch))?;
        require!(entry.vault == vault.key(), VaultError::AssetNotFound);

        // Read token balance
        require!(
            vault_info.key() == entry.asset_vault,
            VaultError::RemainingAccountsMismatch
        );
        let vault_data = vault_info.try_borrow_data()?;
        require!(vault_data.len() >= 72, VaultError::RemainingAccountsMismatch);
        let balance = u64::from_le_bytes(
            vault_data[64..72]
                .try_into()
                .map_err(|_| error!(VaultError::MathOverflow))?,
        );

        // Read OraclePrice
        require!(
            oracle_info.key() == entry.oracle,
            VaultError::InvalidOracle
        );
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
        require!(oracle.price > 0, VaultError::InvalidOracle);

        // Compute this asset's value
        let value = math::asset_value(balance, oracle.price, entry.asset_decimals)?;
        total_value = total_value
            .checked_add(value as u128)
            .ok_or(error!(VaultError::MathOverflow))?;

        // Capture deposited asset's price
        if entry.asset_mint == deposit_mint {
            deposit_price = oracle.price;
        }

        drop(entry_data);
        drop(vault_data);
        drop(oracle_data);
    }

    require!(deposit_price > 0, VaultError::AssetNotFound);

    let total_value =
        u64::try_from(total_value).map_err(|_| error!(VaultError::MathOverflow))?;

    // Compute deposit value in base units
    let deposit_value = math::asset_value(
        amount,
        deposit_price,
        ctx.accounts.asset_entry.asset_decimals,
    )?;

    // Calculate shares to mint
    let total_shares = ctx.accounts.shares_mint.supply;
    let shares = convert_to_shares(
        deposit_value,
        total_value,
        total_shares,
        ctx.accounts.vault.decimals_offset,
        Rounding::Floor,
    )?;

    require!(shares >= min_shares_out, VaultError::SlippageExceeded);

    // Transfer asset from user to vault
    transfer_checked(
        CpiContext::new(
            ctx.accounts.asset_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_asset_account.to_account_info(),
                to: ctx.accounts.asset_vault.to_account_info(),
                mint: ctx.accounts.asset_mint.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.asset_mint.decimals,
    )?;

    // Mint shares to user
    let vault_id_bytes = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = ctx.accounts.vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, &vault_id_bytes, &[bump]]];

    token_2022::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.shares_mint.to_account_info(),
                to: ctx.accounts.user_shares_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        shares,
    )?;

    // Update vault total_shares
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_add(shares)
        .ok_or(VaultError::MathOverflow)?;

    emit!(DepositEvent {
        vault: vault.key(),
        caller: ctx.accounts.user.key(),
        owner: ctx.accounts.user.key(),
        asset_mint: deposit_mint,
        assets: amount,
        shares,
        value: deposit_value,
    });

    Ok(())
}
