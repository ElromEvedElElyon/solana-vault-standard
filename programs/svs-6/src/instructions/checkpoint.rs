//! Checkpoint instruction: permissionless crank that finalizes accrued streaming yield.

use anchor_lang::prelude::*;

use crate::{
    constants::VAULT_SEED,
    error::VaultError,
    events::Checkpoint as CheckpointEvent,
    math::effective_total_assets,
    state::ConfidentialStreamVault,
};

#[derive(Accounts)]
pub struct CheckpointAccounts<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, vault.asset_mint.as_ref(), &vault.vault_id.to_le_bytes()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, ConfidentialStreamVault>,
    // Permissionless - no signer required
}

pub fn handler(ctx: Context<CheckpointAccounts>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let clock = Clock::get()?;
    let now = clock.unix_timestamp;

    // Calculate accrued yield since last checkpoint
    let effective = effective_total_assets(vault, now)?;
    let accrued = effective
        .checked_sub(vault.base_assets)
        .ok_or(VaultError::MathOverflow)?;

    // Early exit if nothing to accrue
    if accrued == 0 {
        return Ok(());
    }

    // Update state
    vault.base_assets = effective;

    if now >= vault.stream_end {
        // Stream complete - clear stream state
        vault.stream_amount = 0;
        vault.stream_start = now;
        vault.stream_end = now;
    } else {
        // Partial checkpoint - reduce remaining stream
        vault.stream_amount = vault
            .stream_amount
            .checked_sub(accrued)
            .ok_or(VaultError::MathOverflow)?;
        vault.stream_start = now;
    }

    vault.last_checkpoint = now;

    emit!(CheckpointEvent {
        vault: vault.key(),
        accrued,
        new_base_assets: vault.base_assets,
        timestamp: now,
    });

    Ok(())
}
