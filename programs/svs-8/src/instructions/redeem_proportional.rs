//! Redeem shares for proportional amounts of all basket assets.

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{self, Burn, Token2022},
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    constants::VAULT_SEED,
    error::VaultError,
    events::Withdraw as WithdrawEvent,
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

    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,

    // remaining_accounts per asset (4 each):
    // [AssetEntry, asset_vault(mut), user_token_account(mut), asset_mint]
}

pub fn handler<'a>(ctx: Context<'_, '_, 'a, 'a, RedeemProportional<'a>>, shares: u64) -> Result<()> {
    require!(shares > 0, VaultError::ZeroAmount);
    require!(
        ctx.accounts.user_shares_account.amount >= shares,
        VaultError::InsufficientShares
    );

    let vault = &ctx.accounts.vault;
    let num_assets = vault.num_assets as usize;
    let remaining = &ctx.remaining_accounts;
    let total_shares = ctx.accounts.shares_mint.supply;

    require!(
        remaining.len() >= num_assets * 4,
        VaultError::RemainingAccountsMismatch
    );
    require!(total_shares > 0, VaultError::InsufficientShares);

    // Burn shares first
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

    let vault_id_bytes = vault.vault_id.to_le_bytes();
    let bump = vault.bump;
    let vault_key = vault.key();

    // Transfer proportional amount of each asset
    for i in 0..num_assets {
        let entry_info = &remaining[i * 4];
        let vault_token_info = &remaining[i * 4 + 1];
        let user_token_info = &remaining[i * 4 + 2];
        let mint_info = &remaining[i * 4 + 3];

        // Read AssetEntry
        require!(
            *entry_info.owner == crate::ID,
            VaultError::RemainingAccountsMismatch
        );
        let entry_data = entry_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &entry_data[..])
            .map_err(|_| error!(VaultError::RemainingAccountsMismatch))?;
        require!(entry.vault == vault_key, VaultError::AssetNotFound);

        // Read vault token balance
        let vault_data = vault_token_info.try_borrow_data()?;
        require!(vault_data.len() >= 72, VaultError::RemainingAccountsMismatch);
        let vault_balance = u64::from_le_bytes(
            vault_data[64..72]
                .try_into()
                .map_err(|_| error!(VaultError::MathOverflow))?,
        );

        // Proportional share: asset_amount = vault_balance * shares / total_shares
        let asset_amount = (vault_balance as u128)
            .checked_mul(shares as u128)
            .ok_or(error!(VaultError::MathOverflow))?
            .checked_div(total_shares as u128)
            .ok_or(error!(VaultError::DivisionByZero))?;
        let asset_amount =
            u64::try_from(asset_amount).map_err(|_| error!(VaultError::MathOverflow))?;

        if asset_amount == 0 {
            drop(entry_data);
            drop(vault_data);
            continue;
        }

        let asset_decimals = entry.asset_decimals;
        drop(entry_data);
        drop(vault_data);

        // Transfer from vault to user via CPI
        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, &vault_id_bytes, &[bump]]];

        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            ctx.accounts.token_program.key,
            vault_token_info.key,
            mint_info.key,
            user_token_info.key,
            &vault_key,
            &[],
            asset_amount,
            asset_decimals,
        )?;

        anchor_lang::solana_program::program::invoke_signed(
            &transfer_ix,
            &[
                vault_token_info.clone(),
                mint_info.clone(),
                user_token_info.clone(),
                ctx.accounts.vault.to_account_info(),
            ],
            signer_seeds,
        )?;
    }

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
        assets: 0,
        shares,
    });

    Ok(())
}
