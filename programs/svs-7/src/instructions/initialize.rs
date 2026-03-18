//! Initialize instruction: create vault PDA, shares mint, and wSOL vault.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::token_2022::{
    spl_token_2022::{
        extension::ExtensionType,
        instruction::{initialize_account3, initialize_mint2},
        native_mint,
        state::{Account as SplTokenAccount, Mint as SplMint},
    },
    Token2022,
};
use anchor_spl::token_interface::{Mint, TokenInterface};

use crate::{
    constants::{SHARES_DECIMALS, SHARES_MINT_SEED, SOL_DECIMALS, VAULT_SEED, WSOL_VAULT_SEED},
    error::VaultError,
    events::VaultInitialized,
    state::NativeSolVault,
};

#[derive(Accounts)]
#[instruction(vault_id: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = NativeSolVault::LEN,
        seeds = [VAULT_SEED, &vault_id.to_le_bytes()],
        bump
    )]
    pub vault: Account<'info, NativeSolVault>,

    /// Native mint (wSOL). Validated in handler.
    pub wsol_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: Shares mint is initialized via CPI in handler
    #[account(
        mut,
        seeds = [SHARES_MINT_SEED, vault.key().as_ref()],
        bump
    )]
    pub shares_mint: UncheckedAccount<'info>,

    /// CHECK: wSOL vault token account created via CPI in handler.
    /// We use a PDA-owned token account so the vault can sign transfers.
    #[account(
        mut,
        seeds = [WSOL_VAULT_SEED, vault.key().as_ref()],
        bump
    )]
    pub wsol_vault: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<Initialize>,
    vault_id: u64,
    name: String,
    symbol: String,
    _uri: String,
) -> Result<()> {
    // Validate that wsol_mint is the native mint
    require!(
        ctx.accounts.wsol_mint.key() == native_mint::id(),
        VaultError::InvalidWsolMint
    );

    let vault_key = ctx.accounts.vault.key();
    let vault_bump = ctx.bumps.vault;
    let shares_mint_bump = ctx.bumps.shares_mint;
    let wsol_vault_bump = ctx.bumps.wsol_vault;

    // --- Create shares mint (Token-2022) ---
    let mint_size = ExtensionType::try_calculate_account_len::<SplMint>(&[])
        .map_err(|_| VaultError::MathOverflow)?;

    let rent = &ctx.accounts.rent;
    let mint_lamports = rent.minimum_balance(mint_size);

    let shares_mint_bump_bytes = [shares_mint_bump];
    let shares_mint_seeds: &[&[u8]] = &[
        SHARES_MINT_SEED,
        vault_key.as_ref(),
        &shares_mint_bump_bytes,
    ];

    invoke_signed(
        &anchor_lang::solana_program::system_instruction::create_account(
            &ctx.accounts.authority.key(),
            &ctx.accounts.shares_mint.key(),
            mint_lamports,
            mint_size as u64,
            &ctx.accounts.token_2022_program.key(),
        ),
        &[
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.shares_mint.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[shares_mint_seeds],
    )?;

    let init_mint_ix = initialize_mint2(
        &ctx.accounts.token_2022_program.key(),
        &ctx.accounts.shares_mint.key(),
        &vault_key,
        None,
        SHARES_DECIMALS,
    )?;

    invoke_signed(
        &init_mint_ix,
        &[ctx.accounts.shares_mint.to_account_info()],
        &[shares_mint_seeds],
    )?;

    // --- Create wSOL vault token account (SPL Token, owned by vault PDA) ---
    // wSOL uses the original SPL Token program, not Token-2022
    let wsol_account_size = SplTokenAccount::LEN;
    let wsol_lamports = rent.minimum_balance(wsol_account_size);

    let wsol_vault_bump_bytes = [wsol_vault_bump];
    let wsol_vault_seeds: &[&[u8]] = &[
        WSOL_VAULT_SEED,
        vault_key.as_ref(),
        &wsol_vault_bump_bytes,
    ];

    invoke_signed(
        &anchor_lang::solana_program::system_instruction::create_account(
            &ctx.accounts.authority.key(),
            &ctx.accounts.wsol_vault.key(),
            wsol_lamports,
            wsol_account_size as u64,
            &ctx.accounts.token_program.key(),
        ),
        &[
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.wsol_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[wsol_vault_seeds],
    )?;

    // Initialize the token account with vault PDA as owner
    let init_account_ix = initialize_account3(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.wsol_vault.key(),
        &ctx.accounts.wsol_mint.key(),
        &vault_key,
    )?;

    invoke_signed(
        &init_account_ix,
        &[
            ctx.accounts.wsol_vault.to_account_info(),
            ctx.accounts.wsol_mint.to_account_info(),
        ],
        &[wsol_vault_seeds],
    )?;

    // --- Set vault state ---
    let vault = &mut ctx.accounts.vault;
    vault.authority = ctx.accounts.authority.key();
    vault.wsol_mint = ctx.accounts.wsol_mint.key();
    vault.shares_mint = ctx.accounts.shares_mint.key();
    vault.wsol_vault = ctx.accounts.wsol_vault.key();
    // SOL has 9 decimals, shares have 9 decimals, offset = 0
    vault.decimals_offset = SOL_DECIMALS
        .checked_sub(SOL_DECIMALS)
        .ok_or(VaultError::MathOverflow)?;
    vault.bump = vault_bump;
    vault.shares_mint_bump = shares_mint_bump;
    vault.wsol_vault_bump = wsol_vault_bump;
    vault.paused = false;
    vault.vault_id = vault_id;
    vault.total_sol = 0;
    vault._reserved = [0u8; 64];

    emit!(VaultInitialized {
        vault: vault.key(),
        authority: vault.authority,
        wsol_mint: vault.wsol_mint,
        shares_mint: vault.shares_mint,
        wsol_vault: vault.wsol_vault,
        vault_id,
    });

    msg!("Native SOL vault initialized: {} ({})", name, symbol);

    Ok(())
}
