//! Deposit SOL instruction: accept native SOL, wrap to wSOL, mint shares.
//!
//! Flow:
//! 1. Transfer SOL from user to wSOL vault PDA via system_program
//! 2. Sync the wSOL token account (SyncNative) to reflect lamport balance
//! 3. Mint proportional shares to user

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::{self, spl_token_2022::instruction::sync_native, MintTo, Token2022},
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    constants::{MIN_DEPOSIT_LAMPORTS, VAULT_SEED, WSOL_VAULT_SEED},
    error::VaultError,
    events::SolDeposited,
    math::{convert_to_shares, Rounding},
    state::NativeSolVault,
};

#[derive(Accounts)]
pub struct DepositSol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Account<'info, NativeSolVault>,

    #[account(
        mut,
        seeds = [WSOL_VAULT_SEED, vault.key().as_ref()],
        bump = vault.wsol_vault_bump,
    )]
    pub wsol_vault: InterfaceAccount<'info, TokenAccount>,

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

    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<DepositSol>, lamports: u64, min_shares_out: u64) -> Result<()> {
    require!(lamports > 0, VaultError::ZeroAmount);
    require!(lamports >= MIN_DEPOSIT_LAMPORTS, VaultError::DepositTooSmall);

    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;
    let total_assets = vault.total_sol;

    // Calculate shares to mint (floor rounding - favors vault)
    let shares = convert_to_shares(
        lamports,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Floor,
    )?;

    // Slippage check
    require!(shares >= min_shares_out, VaultError::SlippageExceeded);

    // 1. Transfer SOL from user to wSOL vault PDA via system_program
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.wsol_vault.key(),
            lamports,
        ),
        &[
            ctx.accounts.user.to_account_info(),
            ctx.accounts.wsol_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;

    // 2. SyncNative updates the wSOL token balance to match the account's lamports
    let sync_ix = sync_native(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.wsol_vault.key(),
    )?;

    anchor_lang::solana_program::program::invoke(
        &sync_ix,
        &[ctx.accounts.wsol_vault.to_account_info()],
    )?;

    // Reload wSOL vault after sync to get accurate balance
    ctx.accounts.wsol_vault.reload()?;

    // 3. Mint shares to user (vault PDA signs as mint authority)
    let vault_id_bytes = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = ctx.accounts.vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
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

    // 4. Update vault accounting
    let vault = &mut ctx.accounts.vault;
    vault.total_sol = vault
        .total_sol
        .checked_add(lamports)
        .ok_or(VaultError::MathOverflow)?;

    emit!(SolDeposited {
        vault: vault.key(),
        depositor: ctx.accounts.user.key(),
        sol_amount: lamports,
        shares_minted: shares,
        total_sol_after: vault.total_sol,
    });

    Ok(())
}
