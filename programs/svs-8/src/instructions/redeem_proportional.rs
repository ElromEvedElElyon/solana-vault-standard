//! Redeem proportional: burn shares and receive proportional amounts of all basket assets.

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{self, Burn, Token2022},
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::MULTI_VAULT_SEED,
    error::VaultError,
    events::ProportionalRedeem,
    state::{AssetEntry, MultiAssetVault},
};

#[derive(Accounts)]
pub struct RedeemProportional<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Account<'info, MultiAssetVault>,

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

    // remaining_accounts layout:
    // [AssetEntry, asset_vault, user_token_account, asset_mint] × num_assets
}

pub fn handler(
    ctx: Context<RedeemProportional>,
    shares: u64,
    min_amounts_out: Vec<u64>,
) -> Result<()> {
    require!(shares > 0, VaultError::ZeroAmount);
    require!(
        ctx.accounts.user_shares_account.amount >= shares,
        VaultError::InsufficientShares
    );

    let vault = &ctx.accounts.vault;
    require!(vault.num_assets > 0, VaultError::NoAssets);

    let num_assets = vault.num_assets as usize;

    // remaining_accounts: [AssetEntry, asset_vault, user_token, asset_mint] × num_assets
    require!(
        ctx.remaining_accounts.len() == num_assets * 4,
        VaultError::InvalidRemainingAccounts
    );
    require!(
        min_amounts_out.len() == num_assets,
        VaultError::InvalidRemainingAccounts
    );

    let total_shares = ctx.accounts.shares_mint.supply;
    let vault_key = vault.key();

    // Burn shares from user FIRST (prevents reentrancy)
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

    // Prepare vault signer seeds
    let vault_id_bytes = vault.vault_id.to_le_bytes();
    let bump = vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[
        MULTI_VAULT_SEED,
        vault_id_bytes.as_ref(),
        &[bump],
    ]];

    // For each asset, transfer proportional amount to user
    for i in 0..num_assets {
        let entry_info = &ctx.remaining_accounts[i * 4];
        let asset_vault_info = &ctx.remaining_accounts[i * 4 + 1];
        let user_token_info = &ctx.remaining_accounts[i * 4 + 2];
        let asset_mint_info = &ctx.remaining_accounts[i * 4 + 3];

        // Validate AssetEntry
        let entry_data = entry_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &entry_data[..])?;
        require!(entry.vault == vault_key, VaultError::AssetNotFound);

        // Read asset vault balance
        let vault_data = asset_vault_info.try_borrow_data()?;
        require!(vault_data.len() >= 72, VaultError::InvalidRemainingAccounts);
        let asset_balance = u64::from_le_bytes(
            vault_data[64..72]
                .try_into()
                .map_err(|_| error!(VaultError::MathOverflow))?,
        );
        drop(vault_data);

        // Calculate proportional share: asset_amount = asset_balance * shares / total_shares
        let asset_amount = (asset_balance as u128)
            .checked_mul(shares as u128)
            .ok_or(error!(VaultError::MathOverflow))?
            .checked_div(total_shares as u128)
            .ok_or(error!(VaultError::DivisionByZero))?;

        let asset_amount_u64 =
            u64::try_from(asset_amount).map_err(|_| error!(VaultError::MathOverflow))?;

        // Slippage check per asset
        require!(
            asset_amount_u64 >= min_amounts_out[i],
            VaultError::SlippageExceeded
        );

        if asset_amount_u64 == 0 {
            continue;
        }

        // Transfer asset to user
        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.asset_token_program.to_account_info(),
                TransferChecked {
                    from: asset_vault_info.to_account_info(),
                    to: user_token_info.to_account_info(),
                    mint: asset_mint_info.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                signer_seeds,
            ),
            asset_amount_u64,
            entry.asset_decimals,
        )?;
    }

    // Update cached total_shares
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_sub(shares)
        .ok_or(VaultError::MathOverflow)?;

    emit!(ProportionalRedeem {
        vault: vault.key(),
        caller: ctx.accounts.user.key(),
        shares,
    });

    Ok(())
}
