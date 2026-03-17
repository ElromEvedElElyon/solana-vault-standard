//! Distribute yield instruction: authority starts a new streaming yield distribution.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::{
    constants::{MIN_STREAM_DURATION, VAULT_SEED},
    error::VaultError,
    events::YieldStreamStarted,
    math::effective_total_assets,
    state::ConfidentialStreamVault,
};

#[derive(Accounts)]
pub struct DistributeYield<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority @ VaultError::Unauthorized,
        has_one = asset_vault,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Account<'info, ConfidentialStreamVault>,

    #[account(
        constraint = asset_mint.key() == vault.asset_mint,
    )]
    pub asset_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub asset_vault: InterfaceAccount<'info, TokenAccount>,

    /// Source of yield tokens (authority's ATA or external protocol)
    #[account(
        mut,
        token::mint = vault.asset_mint,
    )]
    pub yield_source: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(
    ctx: Context<DistributeYield>,
    yield_amount: u64,
    duration: i64,
) -> Result<()> {
    require!(yield_amount > 0, VaultError::ZeroAmount);
    require!(duration >= MIN_STREAM_DURATION, VaultError::StreamTooShort);

    let vault = &mut ctx.accounts.vault;
    let clock = Clock::get()?;
    let now = clock.unix_timestamp;

    // If stream is active, auto-checkpoint to finalize current stream
    if now < vault.stream_end && vault.stream_amount > 0 {
        let accrued = effective_total_assets(vault, now)?
            .checked_sub(vault.base_assets)
            .ok_or(VaultError::MathOverflow)?;

        vault.base_assets = vault
            .base_assets
            .checked_add(accrued)
            .ok_or(VaultError::MathOverflow)?;
        vault.stream_amount = 0;
    }

    // Transfer yield tokens from source to vault
    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.yield_source.to_account_info(),
                to: ctx.accounts.asset_vault.to_account_info(),
                mint: ctx.accounts.asset_mint.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            },
        ),
        yield_amount,
        ctx.accounts.asset_mint.decimals,
    )?;

    // Initialize new stream
    vault.stream_amount = yield_amount;
    vault.stream_start = now;
    vault.stream_end = now
        .checked_add(duration)
        .ok_or(VaultError::MathOverflow)?;
    vault.last_checkpoint = now;

    emit!(YieldStreamStarted {
        vault: vault.key(),
        amount: yield_amount,
        duration,
        start: now,
        end: vault.stream_end,
    });

    Ok(())
}
