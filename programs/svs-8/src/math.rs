//! Multi-asset vault math: portfolio valuation, share conversion, oracle pricing.

use anchor_lang::prelude::*;

use crate::error::VaultError;
use crate::state::{AssetEntry, OraclePrice};

pub use svs_math::Rounding;

/// Convert deposit value to shares using total portfolio value as denominator.
pub fn convert_to_shares(
    deposit_value: u64,
    total_value: u64,
    total_shares: u64,
    decimals_offset: u8,
    rounding: Rounding,
) -> Result<u64> {
    let offset = 10u64
        .checked_pow(decimals_offset as u32)
        .ok_or(error!(VaultError::MathOverflow))?;

    svs_math::mul_div(
        deposit_value,
        total_shares.checked_add(offset).ok_or(error!(VaultError::MathOverflow))?,
        total_value.checked_add(1).ok_or(error!(VaultError::MathOverflow))?,
        rounding,
    )
    .map_err(|_| error!(VaultError::MathOverflow))
}

/// Convert shares to asset value using total portfolio value.
pub fn convert_to_assets(
    shares: u64,
    total_value: u64,
    total_shares: u64,
    decimals_offset: u8,
    rounding: Rounding,
) -> Result<u64> {
    let offset = 10u64
        .checked_pow(decimals_offset as u32)
        .ok_or(error!(VaultError::MathOverflow))?;

    svs_math::mul_div(
        shares,
        total_value.checked_add(1).ok_or(error!(VaultError::MathOverflow))?,
        total_shares.checked_add(offset).ok_or(error!(VaultError::MathOverflow))?,
        rounding,
    )
    .map_err(|_| error!(VaultError::MathOverflow))
}

/// Calculate total portfolio value in base units by summing all asset values.
pub fn total_portfolio_value(
    asset_entries: &[AssetEntry],
    balances: &[u64],
    prices: &[u64],
) -> Result<u64> {
    require!(
        asset_entries.len() == balances.len() && balances.len() == prices.len(),
        VaultError::InvalidRemainingAccounts
    );

    let mut total: u128 = 0;
    for i in 0..asset_entries.len() {
        let value = (balances[i] as u128)
            .checked_mul(prices[i] as u128)
            .ok_or(error!(VaultError::MathOverflow))?
            .checked_div(
                10u128
                    .checked_pow(asset_entries[i].asset_decimals as u32)
                    .ok_or(error!(VaultError::MathOverflow))?,
            )
            .ok_or(error!(VaultError::DivisionByZero))?;
        total = total
            .checked_add(value)
            .ok_or(error!(VaultError::MathOverflow))?;
    }

    u64::try_from(total).map_err(|_| error!(VaultError::MathOverflow))
}

/// Read and validate oracle price from an OraclePrice account.
pub fn read_oracle_price(
    oracle_account: &AccountInfo,
    expected_mint: &Pubkey,
    max_staleness_secs: u64,
    program_id: &Pubkey,
) -> Result<u64> {
    // Validate oracle account owner
    require!(
        oracle_account.owner == program_id,
        VaultError::InvalidOracle
    );

    let data = oracle_account.try_borrow_data()?;
    // Skip 8-byte discriminator
    let oracle: OraclePrice = OraclePrice::try_deserialize(&mut &data[..])?;

    // Validate asset mint matches
    require!(
        oracle.asset_mint == *expected_mint,
        VaultError::AssetMintMismatch
    );

    // Validate freshness
    let clock = Clock::get()?;
    let age = clock
        .unix_timestamp
        .checked_sub(oracle.updated_at)
        .ok_or(error!(VaultError::MathOverflow))?;

    require!(age >= 0, VaultError::OracleStale);
    require!(
        (age as u64) <= max_staleness_secs,
        VaultError::OracleStale
    );

    require!(oracle.price > 0, VaultError::InvalidOracle);

    Ok(oracle.price)
}
