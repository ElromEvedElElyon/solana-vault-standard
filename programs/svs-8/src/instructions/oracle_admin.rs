//! Oracle admin: initialize and update oracle price accounts for testing.
//! In production, use Pyth or Switchboard oracles directly.

use anchor_lang::prelude::*;

use crate::{
    error::VaultError,
    state::OraclePrice,
};

pub const ORACLE_PRICE_SEED: &[u8] = b"oracle_price";

#[derive(Accounts)]
pub struct InitializeOracle<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Asset mint to create oracle for
    pub asset_mint: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        space = OraclePrice::LEN,
        seeds = [ORACLE_PRICE_SEED, asset_mint.key().as_ref()],
        bump,
    )]
    pub oracle_price: Account<'info, OraclePrice>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_oracle(
    ctx: Context<InitializeOracle>,
    initial_price: u64,
) -> Result<()> {
    require!(initial_price > 0, VaultError::InvalidOracle);

    let clock = Clock::get()?;
    let oracle = &mut ctx.accounts.oracle_price;
    oracle.asset_mint = ctx.accounts.asset_mint.key();
    oracle.price = initial_price;
    oracle.updated_at = clock.unix_timestamp;
    oracle.authority = ctx.accounts.authority.key();
    oracle.bump = ctx.bumps.oracle_price;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateOracle<'info> {
    #[account(
        constraint = authority.key() == oracle_price.authority @ VaultError::Unauthorized,
    )]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub oracle_price: Account<'info, OraclePrice>,
}

pub fn update_oracle(ctx: Context<UpdateOracle>, new_price: u64) -> Result<()> {
    require!(new_price > 0, VaultError::InvalidOracle);

    let clock = Clock::get()?;
    let oracle = &mut ctx.accounts.oracle_price;
    oracle.price = new_price;
    oracle.updated_at = clock.unix_timestamp;

    Ok(())
}
