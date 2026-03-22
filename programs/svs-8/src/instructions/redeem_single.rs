//! Redeem single: burn shares and receive one specific basket asset.

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{self, Burn, Token2022},
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::{ASSET_ENTRY_SEED, MAX_ORACLE_STALENESS, MULTI_VAULT_SEED},
    error::VaultError,
    events::SingleRedeem,
    math::{convert_to_assets, read_oracle_price, total_portfolio_value, Rounding},
    state::{AssetEntry, MultiAssetVault},
};

#[derive(Accounts)]
pub struct RedeemSingle<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Account<'info, MultiAssetVault>,

    /// The specific asset to receive
    pub redeem_asset_mint: InterfaceAccount<'info, Mint>,

    /// AssetEntry for the redeemed asset
    #[account(
        seeds = [ASSET_ENTRY_SEED, vault.key().as_ref(), redeem_asset_mint.key().as_ref()],
        bump = redeem_asset_entry.bump,
        constraint = redeem_asset_entry.vault == vault.key() @ VaultError::AssetNotFound,
    )]
    pub redeem_asset_entry: Account<'info, AssetEntry>,

    /// User's token account for receiving the asset
    #[account(
        mut,
        constraint = user_asset_account.mint == redeem_asset_mint.key(),
        constraint = user_asset_account.owner == user.key(),
    )]
    pub user_asset_account: InterfaceAccount<'info, TokenAccount>,

    /// Vault's token account for the redeemed asset
    #[account(
        mut,
        constraint = redeem_asset_vault.key() == redeem_asset_entry.asset_vault,
    )]
    pub redeem_asset_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = shares_mint.key() == vault.shares_mint,
    )]
    pub shares_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        constraint = user_shares_account.mint == vault.shares_mint,
        constraint = user_shares_account.owner == user.key(),
    )]
    pub user_shares_account: InterfaceAccount<'info, TokenAccount>,

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
    require!(vault.num_assets > 0, VaultError::NoAssets);

    let num_assets = vault.num_assets as usize;
    require!(
        ctx.remaining_accounts.len() == num_assets * 3,
        VaultError::InvalidRemainingAccounts
    );

    // Read all asset entries, balances, prices for total portfolio value
    let vault_key = vault.key();
    let mut asset_entries = Vec::with_capacity(num_assets);
    let mut balances = Vec::with_capacity(num_assets);
    let mut prices = Vec::with_capacity(num_assets);

    for i in 0..num_assets {
        let entry_info = &ctx.remaining_accounts[i * 3];
        let vault_info = &ctx.remaining_accounts[i * 3 + 1];
        let oracle_info = &ctx.remaining_accounts[i * 3 + 2];

        let entry_data = entry_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &entry_data[..])?;
        require!(entry.vault == vault_key, VaultError::AssetNotFound);

        let vault_data = vault_info.try_borrow_data()?;
        require!(vault_data.len() >= 72, VaultError::InvalidRemainingAccounts);
        let balance = u64::from_le_bytes(
            vault_data[64..72]
                .try_into()
                .map_err(|_| error!(VaultError::MathOverflow))?,
        );

        let price = read_oracle_price(
            oracle_info,
            &entry.asset_mint,
            MAX_ORACLE_STALENESS,
            ctx.program_id,
        )?;

        asset_entries.push(entry);
        balances.push(balance);
        prices.push(price);
    }

    let total_value = total_portfolio_value(&asset_entries, &balances, &prices)?;
    let total_shares = ctx.accounts.shares_mint.supply;

    // Calculate value of shares being redeemed (floor - favors vault)
    let redeem_value = convert_to_assets(
        shares,
        total_value,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    // Convert redeem_value to asset amount using oracle price
    let redeem_entry = &ctx.accounts.redeem_asset_entry;
    let asset_price = read_oracle_price(
        &ctx.remaining_accounts
            .iter()
            .enumerate()
            .find(|(i, _)| {
                i % 3 == 0 && {
                    let data = ctx.remaining_accounts[*i].try_borrow_data().ok();
                    data.and_then(|d| AssetEntry::try_deserialize(&mut &d[..]).ok())
                        .map(|e| e.asset_mint == redeem_entry.asset_mint)
                        .unwrap_or(false)
                }
            })
            .map(|(i, _)| &ctx.remaining_accounts[i + 2])
            .ok_or(error!(VaultError::AssetNotFound))?,
        &redeem_entry.asset_mint,
        MAX_ORACLE_STALENESS,
        ctx.program_id,
    )?;

    // asset_amount = redeem_value * 10^asset_decimals / price
    let asset_amount = (redeem_value as u128)
        .checked_mul(
            10u128
                .checked_pow(redeem_entry.asset_decimals as u32)
                .ok_or(error!(VaultError::MathOverflow))?,
        )
        .ok_or(error!(VaultError::MathOverflow))?
        .checked_div(asset_price as u128)
        .ok_or(error!(VaultError::DivisionByZero))?;

    let asset_amount_u64 =
        u64::try_from(asset_amount).map_err(|_| error!(VaultError::MathOverflow))?;

    // Slippage check
    require!(asset_amount_u64 >= min_assets_out, VaultError::SlippageExceeded);

    // Check vault has enough of this asset
    require!(
        asset_amount_u64 <= ctx.accounts.redeem_asset_vault.amount,
        VaultError::InsufficientAssets
    );

    // Burn shares from user
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

    // Transfer asset to user
    let vault_id_bytes = vault.vault_id.to_le_bytes();
    let bump = vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[
        MULTI_VAULT_SEED,
        vault_id_bytes.as_ref(),
        &[bump],
    ]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.asset_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.redeem_asset_vault.to_account_info(),
                to: ctx.accounts.user_asset_account.to_account_info(),
                mint: ctx.accounts.redeem_asset_mint.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        asset_amount_u64,
        ctx.accounts.redeem_asset_mint.decimals,
    )?;

    // Update cached total_shares
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_sub(shares)
        .ok_or(VaultError::MathOverflow)?;

    emit!(SingleRedeem {
        vault: vault.key(),
        caller: ctx.accounts.user.key(),
        asset_mint: ctx.accounts.redeem_asset_mint.key(),
        shares,
        amount: asset_amount_u64,
    });

    Ok(())
}
