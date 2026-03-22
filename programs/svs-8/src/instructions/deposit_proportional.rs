//! Deposit proportional: deposit all basket assets in target weight proportions.

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::{self, MintTo, Token2022},
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::{BPS_DENOMINATOR, MAX_ORACLE_STALENESS, MULTI_VAULT_SEED},
    error::VaultError,
    events::ProportionalDeposit,
    math::{convert_to_shares, read_oracle_price, total_portfolio_value, Rounding},
    state::{AssetEntry, MultiAssetVault},
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
        init_if_needed,
        payer = user,
        associated_token::mint = shares_mint,
        associated_token::authority = user,
        associated_token::token_program = token_2022_program,
    )]
    pub user_shares_account: InterfaceAccount<'info, TokenAccount>,

    pub asset_token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    // remaining_accounts layout (for ALL assets):
    // [AssetEntry, asset_vault, oracle, user_token_account, asset_mint] × num_assets
}

pub fn handler(
    ctx: Context<DepositProportional>,
    base_amount: u64,
    min_shares_out: u64,
) -> Result<()> {
    require!(base_amount > 0, VaultError::ZeroAmount);

    let vault = &ctx.accounts.vault;
    require!(vault.num_assets > 0, VaultError::NoAssets);

    let num_assets = vault.num_assets as usize;

    // remaining_accounts: [AssetEntry, asset_vault, oracle, user_token, asset_mint] × num_assets
    require!(
        ctx.remaining_accounts.len() == num_assets * 5,
        VaultError::InvalidRemainingAccounts
    );

    let vault_key = vault.key();

    // First pass: read all entries, balances, prices for portfolio value
    let mut asset_entries = Vec::with_capacity(num_assets);
    let mut balances = Vec::with_capacity(num_assets);
    let mut prices = Vec::with_capacity(num_assets);

    for i in 0..num_assets {
        let entry_info = &ctx.remaining_accounts[i * 5];
        let vault_info = &ctx.remaining_accounts[i * 5 + 1];
        let oracle_info = &ctx.remaining_accounts[i * 5 + 2];

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

    // Calculate total portfolio value before deposit
    let total_value_before = total_portfolio_value(&asset_entries, &balances, &prices)?;
    let total_shares = ctx.accounts.shares_mint.supply;

    // Second pass: transfer proportional amounts for each asset
    let mut total_deposit_value: u128 = 0;

    for i in 0..num_assets {
        let entry = &asset_entries[i];
        let asset_vault_info = &ctx.remaining_accounts[i * 5 + 1];
        let user_token_info = &ctx.remaining_accounts[i * 5 + 3];
        let asset_mint_info = &ctx.remaining_accounts[i * 5 + 4];

        // Calculate proportional amount based on target weight
        let asset_amount = (base_amount as u128)
            .checked_mul(entry.target_weight_bps as u128)
            .ok_or(error!(VaultError::MathOverflow))?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(error!(VaultError::DivisionByZero))?;

        let asset_amount_u64 =
            u64::try_from(asset_amount).map_err(|_| error!(VaultError::MathOverflow))?;

        if asset_amount_u64 == 0 {
            continue;
        }

        // Calculate value of this deposit
        let value = asset_amount
            .checked_mul(prices[i] as u128)
            .ok_or(error!(VaultError::MathOverflow))?
            .checked_div(
                10u128
                    .checked_pow(entry.asset_decimals as u32)
                    .ok_or(error!(VaultError::MathOverflow))?,
            )
            .ok_or(error!(VaultError::DivisionByZero))?;

        total_deposit_value = total_deposit_value
            .checked_add(value)
            .ok_or(error!(VaultError::MathOverflow))?;

        // Transfer asset from user to vault
        transfer_checked(
            CpiContext::new(
                ctx.accounts.asset_token_program.to_account_info(),
                TransferChecked {
                    from: user_token_info.to_account_info(),
                    to: asset_vault_info.to_account_info(),
                    mint: asset_mint_info.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            asset_amount_u64,
            entry.asset_decimals,
        )?;
    }

    let total_deposit_value_u64 =
        u64::try_from(total_deposit_value).map_err(|_| error!(VaultError::MathOverflow))?;

    // Calculate shares to mint
    let shares = convert_to_shares(
        total_deposit_value_u64,
        total_value_before,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    require!(shares >= min_shares_out, VaultError::SlippageExceeded);

    // Mint shares to user
    let vault_id_bytes = vault.vault_id.to_le_bytes();
    let bump = vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[
        MULTI_VAULT_SEED,
        vault_id_bytes.as_ref(),
        &[bump],
    ]];

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

    // Update cached total_shares
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_add(shares)
        .ok_or(VaultError::MathOverflow)?;

    emit!(ProportionalDeposit {
        vault: vault.key(),
        caller: ctx.accounts.user.key(),
        shares,
        total_value: total_deposit_value_u64,
    });

    Ok(())
}
