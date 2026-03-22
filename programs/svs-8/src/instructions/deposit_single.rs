//! Deposit single asset: deposit one basket asset, mint shares based on oracle value.

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::{self, MintTo, Token2022},
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::{ASSET_ENTRY_SEED, MAX_ORACLE_STALENESS, MIN_DEPOSIT_AMOUNT, MULTI_VAULT_SEED},
    error::VaultError,
    events::SingleDeposit,
    math::{convert_to_shares, read_oracle_price, total_portfolio_value, Rounding},
    state::{AssetEntry, MultiAssetVault},
};

#[derive(Accounts)]
pub struct DepositSingle<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Account<'info, MultiAssetVault>,

    /// The specific asset being deposited
    pub deposit_asset_mint: InterfaceAccount<'info, Mint>,

    /// AssetEntry for the deposited asset
    #[account(
        seeds = [ASSET_ENTRY_SEED, vault.key().as_ref(), deposit_asset_mint.key().as_ref()],
        bump = deposit_asset_entry.bump,
        constraint = deposit_asset_entry.vault == vault.key() @ VaultError::AssetNotFound,
    )]
    pub deposit_asset_entry: Account<'info, AssetEntry>,

    /// User's token account for the deposited asset
    #[account(
        mut,
        constraint = user_asset_account.mint == deposit_asset_mint.key(),
        constraint = user_asset_account.owner == user.key(),
    )]
    pub user_asset_account: InterfaceAccount<'info, TokenAccount>,

    /// Vault's token account for the deposited asset
    #[account(
        mut,
        constraint = deposit_asset_vault.key() == deposit_asset_entry.asset_vault,
    )]
    pub deposit_asset_vault: InterfaceAccount<'info, TokenAccount>,

    /// Shares mint
    #[account(
        mut,
        constraint = shares_mint.key() == vault.shares_mint,
    )]
    pub shares_mint: InterfaceAccount<'info, Mint>,

    /// User's shares token account (init_if_needed)
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

    // remaining_accounts layout (for ALL assets in basket):
    // [AssetEntry, asset_vault (TokenAccount), oracle] × num_assets
    // This is needed to compute total_portfolio_value
}

pub fn handler(ctx: Context<DepositSingle>, amount: u64, min_shares_out: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(amount >= MIN_DEPOSIT_AMOUNT, VaultError::DepositTooSmall);

    let vault = &ctx.accounts.vault;
    require!(vault.num_assets > 0, VaultError::NoAssets);

    let num_assets = vault.num_assets as usize;

    // remaining_accounts: [AssetEntry, asset_vault, oracle] × num_assets
    require!(
        ctx.remaining_accounts.len() == num_assets * 3,
        VaultError::InvalidRemainingAccounts
    );

    // Read all asset entries, balances, and prices
    let mut asset_entries = Vec::with_capacity(num_assets);
    let mut balances = Vec::with_capacity(num_assets);
    let mut prices = Vec::with_capacity(num_assets);

    let vault_key = vault.key();

    for i in 0..num_assets {
        let entry_info = &ctx.remaining_accounts[i * 3];
        let vault_info = &ctx.remaining_accounts[i * 3 + 1];
        let oracle_info = &ctx.remaining_accounts[i * 3 + 2];

        // Deserialize and validate AssetEntry
        let entry_data = entry_info.try_borrow_data()?;
        let entry = AssetEntry::try_deserialize(&mut &entry_data[..])?;
        require!(entry.vault == vault_key, VaultError::AssetNotFound);

        // Read balance from asset vault
        let vault_data = vault_info.try_borrow_data()?;
        // Token account data: skip 0..64 header, amount at offset 64
        require!(vault_data.len() >= 72, VaultError::InvalidRemainingAccounts);
        let balance = u64::from_le_bytes(
            vault_data[64..72]
                .try_into()
                .map_err(|_| error!(VaultError::MathOverflow))?,
        );

        // Read oracle price
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
    let total_value = total_portfolio_value(&asset_entries, &balances, &prices)?;
    let total_shares = ctx.accounts.shares_mint.supply;

    // Calculate value of deposited asset
    let deposit_entry = &ctx.accounts.deposit_asset_entry;
    let deposit_price = read_oracle_price(
        &ctx.remaining_accounts
            .iter()
            .enumerate()
            .find(|(i, _)| {
                i % 3 == 0 && {
                    let data = ctx.remaining_accounts[*i].try_borrow_data().ok();
                    data.and_then(|d| AssetEntry::try_deserialize(&mut &d[..]).ok())
                        .map(|e| e.asset_mint == deposit_entry.asset_mint)
                        .unwrap_or(false)
                }
            })
            .map(|(i, _)| &ctx.remaining_accounts[i + 2])
            .ok_or(error!(VaultError::AssetNotFound))?,
        &deposit_entry.asset_mint,
        MAX_ORACLE_STALENESS,
        ctx.program_id,
    )?;

    let deposit_value = (amount as u128)
        .checked_mul(deposit_price as u128)
        .ok_or(error!(VaultError::MathOverflow))?
        .checked_div(
            10u128
                .checked_pow(deposit_entry.asset_decimals as u32)
                .ok_or(error!(VaultError::MathOverflow))?,
        )
        .ok_or(error!(VaultError::DivisionByZero))?;

    let deposit_value_u64 =
        u64::try_from(deposit_value).map_err(|_| error!(VaultError::MathOverflow))?;

    // Calculate shares to mint (floor rounding - favors vault)
    let shares = convert_to_shares(
        deposit_value_u64,
        total_value,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    // Slippage check
    require!(shares >= min_shares_out, VaultError::SlippageExceeded);

    // Transfer asset from user to vault
    transfer_checked(
        CpiContext::new(
            ctx.accounts.asset_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_asset_account.to_account_info(),
                to: ctx.accounts.deposit_asset_vault.to_account_info(),
                mint: ctx.accounts.deposit_asset_mint.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.deposit_asset_mint.decimals,
    )?;

    // Prepare vault signer seeds
    let vault_id_bytes = vault.vault_id.to_le_bytes();
    let bump = vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[
        MULTI_VAULT_SEED,
        vault_id_bytes.as_ref(),
        &[bump],
    ]];

    // Mint shares to user
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

    emit!(SingleDeposit {
        vault: vault.key(),
        caller: ctx.accounts.user.key(),
        asset_mint: ctx.accounts.deposit_asset_mint.key(),
        amount,
        shares,
        value: deposit_value_u64,
    });

    Ok(())
}
