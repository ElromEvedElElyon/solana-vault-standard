//! Withdraw SOL instruction: burn shares, unwrap wSOL, return native SOL.
//!
//! Flow:
//! 1. Calculate SOL amount from shares (floor rounding - favors vault)
//! 2. Burn shares from user
//! 3. Transfer wSOL from vault to user's ephemeral wSOL ATA
//! 4. Close user's wSOL ATA, sending all lamports (including wSOL) to user
//!
//! The user ends up with native SOL. The ephemeral wSOL ATA is created
//! (init_if_needed) and closed in the same transaction.

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::{self, Burn, Token2022},
    token_interface::{
        transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
    },
};

use crate::{
    constants::{VAULT_SEED, WSOL_VAULT_SEED},
    error::VaultError,
    events::SolWithdrawn,
    math::{convert_to_assets, Rounding},
    state::NativeSolVault,
};

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Account<'info, NativeSolVault>,

    /// wSOL mint (native mint)
    #[account(
        constraint = wsol_mint.key() == vault.wsol_mint,
    )]
    pub wsol_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [WSOL_VAULT_SEED, vault.key().as_ref()],
        bump = vault.wsol_vault_bump,
    )]
    pub wsol_vault: InterfaceAccount<'info, TokenAccount>,

    /// User's ephemeral wSOL token account.
    /// Created if needed, then closed at end to unwrap SOL to user.
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = wsol_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_wsol_account: InterfaceAccount<'info, TokenAccount>,

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
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Redeem shares for native SOL (floor rounding - protects vault).
///
/// Burns the specified shares, calculates proportional SOL, transfers wSOL
/// to an ephemeral account, then closes it to unwrap back to native SOL.
pub fn handler(ctx: Context<WithdrawSol>, shares: u64, min_sol_out: u64) -> Result<()> {
    require!(shares > 0, VaultError::ZeroAmount);

    require!(
        ctx.accounts.user_shares_account.amount >= shares,
        VaultError::InsufficientShares
    );

    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let total_assets = vault.total_sol;

    // Calculate SOL to return (floor rounding - user gets less, protects vault)
    let sol_amount = convert_to_assets(
        shares,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    // Slippage check
    require!(sol_amount >= min_sol_out, VaultError::SlippageExceeded);

    // Check vault has enough SOL
    require!(sol_amount <= total_assets, VaultError::InsufficientAssets);

    // 1. Burn shares from user
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

    // 2. Transfer wSOL from vault to user's ephemeral wSOL account
    let vault_id_bytes = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = ctx.accounts.vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        vault_id_bytes.as_ref(),
        &[bump],
    ]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.wsol_vault.to_account_info(),
                to: ctx.accounts.user_wsol_account.to_account_info(),
                mint: ctx.accounts.wsol_mint.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        sol_amount,
        ctx.accounts.wsol_mint.decimals,
    )?;

    // Reload after CPI
    ctx.accounts.wsol_vault.reload()?;

    // 3. Close user's wSOL account, sending all lamports (wSOL + rent) to user
    // This is the standard unwrap pattern: close wSOL account -> user gets native SOL
    anchor_spl::token_interface::close_account(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.user_wsol_account.to_account_info(),
            destination: ctx.accounts.user.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        },
    ))?;

    // 4. Update vault accounting
    let vault = &mut ctx.accounts.vault;
    vault.total_sol = vault
        .total_sol
        .checked_sub(sol_amount)
        .ok_or(VaultError::MathOverflow)?;

    emit!(SolWithdrawn {
        vault: vault.key(),
        withdrawer: ctx.accounts.user.key(),
        sol_amount,
        shares_burned: shares,
        total_sol_after: vault.total_sol,
    });

    Ok(())
}
