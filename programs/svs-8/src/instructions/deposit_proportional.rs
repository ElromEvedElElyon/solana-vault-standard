//! Deposit all basket assets proportionally and receive shares.

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{self, MintTo, Token2022},
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    constants::{MAX_ORACLE_STALENESS, VAULT_SEED, WEIGHT_BPS_DENOMINATOR},
    error::VaultError,
    events::Deposit as DepositEvent,
    math::{self, convert_to_shares, Rounding},
    state::{AssetEntry, MultiAssetVault, OraclePrice},
};

#[derive(Accounts)]
pub struct DepositProportional<'info> {
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

    // remaining_accounts per asset (5 each):
    // [AssetEntry, asset_vault(mut), user_token_account(mut), oracle, asset_mint]
}

pub fn handler<'a>(
    ctx: Context<'_, '_, 'a, 'a, DepositProportional<'a>>,
    base_amount: u64,
    min_shares_out: u64,
) -> Result<()> {
    require!(base_amount > 0, VaultError::ZeroAmount);

    let vault = &ctx.accounts.vault;
    let num_assets = vault.num_assets as usize;
    let remaining = &ctx.remaining_accounts;

    require!(
        remaining.len() >= num_assets * 5,
        VaultError::RemainingAccountsMismatch
    );

    let clock = Clock::get()?;
    let mut total_deposit_value: u128 = 0;
    let mut total_portfolio_value: u128 = 0;

    let vault_id_bytes = vault.vault_id.to_le_bytes();
    let bump = vault.bump;
    let vault_key = vault.key();

    // Process each asset: compute amount, transfer, accumulate values
    for i in 0..num_assets {
        let entry_info = &remaining[i * 5];
        let vault_token_info = &remaining[i * 5 + 1];
        let user_token_info = &remaining[i * 5 + 2];
        let oracle_info = &remaining[i * 5 + 3];
        let mint_info = &remaining[i * 5 + 4];

        // Read AssetEntry
        require!(
            *entry_info.owner == crate::ID,
            VaultError::RemainingAccountsMismatch
        );
        let entry_data = entry_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &entry_data[..])
            .map_err(|_| error!(VaultError::RemainingAccountsMismatch))?;
        require!(entry.vault == vault_key, VaultError::AssetNotFound);

        // Read current vault balance
        let vault_data = vault_token_info.try_borrow_data()?;
        require!(vault_data.len() >= 72, VaultError::RemainingAccountsMismatch);
        let current_balance = u64::from_le_bytes(
            vault_data[64..72]
                .try_into()
                .map_err(|_| error!(VaultError::MathOverflow))?,
        );

        // Read oracle price
        require!(oracle_info.key() == entry.oracle, VaultError::InvalidOracle);
        let oracle_data = oracle_info.try_borrow_data()?;
        let oracle = OraclePrice::try_deserialize(&mut &oracle_data[..])
            .map_err(|_| error!(VaultError::InvalidOracle))?;
        require!(
            clock.unix_timestamp - oracle.updated_at <= MAX_ORACLE_STALENESS as i64,
            VaultError::OracleStale
        );

        // Current portfolio value contribution
        let existing_value = math::asset_value(current_balance, oracle.price, entry.asset_decimals)?;
        total_portfolio_value = total_portfolio_value
            .checked_add(existing_value as u128)
            .ok_or(error!(VaultError::MathOverflow))?;

        // Calculate proportional deposit amount
        let asset_amount = (base_amount as u128)
            .checked_mul(entry.target_weight_bps as u128)
            .ok_or(error!(VaultError::MathOverflow))?
            .checked_div(WEIGHT_BPS_DENOMINATOR as u128)
            .ok_or(error!(VaultError::DivisionByZero))?;
        let asset_amount =
            u64::try_from(asset_amount).map_err(|_| error!(VaultError::MathOverflow))?;

        if asset_amount == 0 {
            drop(entry_data);
            drop(vault_data);
            drop(oracle_data);
            continue;
        }

        // Deposit value
        let deposit_value = math::asset_value(asset_amount, oracle.price, entry.asset_decimals)?;
        total_deposit_value = total_deposit_value
            .checked_add(deposit_value as u128)
            .ok_or(error!(VaultError::MathOverflow))?;

        // Drop borrows before CPI
        let asset_decimals = entry.asset_decimals;
        drop(entry_data);
        drop(vault_data);
        drop(oracle_data);

        // Transfer asset from user to vault
        let transfer_ix = anchor_lang::solana_program::instruction::Instruction {
            program_id: *ctx.accounts.token_program.key,
            accounts: vec![
                AccountMeta::new(*user_token_info.key, false),
                AccountMeta::new_readonly(*mint_info.key, false),
                AccountMeta::new(*vault_token_info.key, false),
                AccountMeta::new_readonly(*ctx.accounts.user.key, true),
            ],
            data: spl_token_2022::instruction::transfer_checked(
                ctx.accounts.token_program.key,
                user_token_info.key,
                mint_info.key,
                vault_token_info.key,
                ctx.accounts.user.key,
                &[],
                asset_amount,
                asset_decimals,
            )?
            .data,
        };

        anchor_lang::solana_program::program::invoke(
            &transfer_ix,
            &[
                user_token_info.clone(),
                mint_info.clone(),
                vault_token_info.clone(),
                ctx.accounts.user.to_account_info(),
            ],
        )?;
    }

    let total_deposit_value =
        u64::try_from(total_deposit_value).map_err(|_| error!(VaultError::MathOverflow))?;
    let total_portfolio_value =
        u64::try_from(total_portfolio_value).map_err(|_| error!(VaultError::MathOverflow))?;

    // Calculate shares
    let total_shares = ctx.accounts.shares_mint.supply;
    let shares = convert_to_shares(
        total_deposit_value,
        total_portfolio_value,
        total_shares,
        ctx.accounts.vault.decimals_offset,
        Rounding::Floor,
    )?;

    require!(shares >= min_shares_out, VaultError::SlippageExceeded);

    // Mint shares
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

    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_add(shares)
        .ok_or(VaultError::MathOverflow)?;

    emit!(DepositEvent {
        vault: vault.key(),
        caller: ctx.accounts.user.key(),
        owner: ctx.accounts.user.key(),
        asset_mint: Pubkey::default(),
        assets: base_amount,
        shares,
        value: total_deposit_value,
    });

    Ok(())
}
