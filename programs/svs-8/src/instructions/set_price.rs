//! Set oracle price for an asset (authority only).

use anchor_lang::prelude::*;

use crate::{
    constants::ORACLE_SEED,
    error::VaultError,
    events::PriceUpdated,
    state::{MultiAssetVault, OraclePrice},
};

#[derive(Accounts)]
pub struct SetPrice<'info> {
    pub authority: Signer<'info>,

    #[account(has_one = authority @ VaultError::Unauthorized)]
    pub vault: Account<'info, MultiAssetVault>,

    #[account(
        mut,
        seeds = [ORACLE_SEED, vault.key().as_ref(), oracle_price.asset_mint.as_ref()],
        bump = oracle_price.bump,
        constraint = oracle_price.vault == vault.key() @ VaultError::InvalidOracle,
    )]
    pub oracle_price: Account<'info, OraclePrice>,
}

pub fn handler(ctx: Context<SetPrice>, price: u64) -> Result<()> {
    require!(price > 0, VaultError::InvalidOracle);

    let oracle = &mut ctx.accounts.oracle_price;
    oracle.price = price;
    oracle.updated_at = Clock::get()?.unix_timestamp;

    emit!(PriceUpdated {
        vault: ctx.accounts.vault.key(),
        asset_mint: oracle.asset_mint,
        price,
    });

    Ok(())
}
