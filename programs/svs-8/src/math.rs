//! Math module - re-exports from svs-math with Anchor error conversion,
//! plus multi-asset portfolio valuation for SVS-8.

use anchor_lang::prelude::*;

use crate::error::VaultError;

pub use svs_math::Rounding;

/// Convert assets to shares with virtual offset protection against inflation attacks.
pub fn convert_to_shares(
    deposit_value: u64,
    total_value: u64,
    total_shares: u64,
    decimals_offset: u8,
    rounding: Rounding,
) -> Result<u64> {
    svs_math::convert_to_shares(
        deposit_value,
        total_value,
        total_shares,
        decimals_offset,
        rounding,
    )
    .map_err(|e| match e {
        svs_math::MathError::Overflow => VaultError::MathOverflow.into(),
        svs_math::MathError::DivisionByZero => VaultError::DivisionByZero.into(),
    })
}

/// Convert shares to assets with virtual offset protection.
pub fn convert_to_assets(
    shares: u64,
    total_value: u64,
    total_shares: u64,
    decimals_offset: u8,
    rounding: Rounding,
) -> Result<u64> {
    svs_math::convert_to_assets(
        shares,
        total_value,
        total_shares,
        decimals_offset,
        rounding,
    )
    .map_err(|e| match e {
        svs_math::MathError::Overflow => VaultError::MathOverflow.into(),
        svs_math::MathError::DivisionByZero => VaultError::DivisionByZero.into(),
    })
}

/// Safe multiplication then division with configurable rounding.
pub fn mul_div(value: u64, numerator: u64, denominator: u64, rounding: Rounding) -> Result<u64> {
    svs_math::mul_div(value, numerator, denominator, rounding).map_err(|e| match e {
        svs_math::MathError::Overflow => VaultError::MathOverflow.into(),
        svs_math::MathError::DivisionByZero => VaultError::DivisionByZero.into(),
    })
}

/// Compute single asset value in base units.
///
/// value = balance * price / 10^asset_decimals
pub fn asset_value(balance: u64, price: u64, asset_decimals: u8) -> Result<u64> {
    let value = (balance as u128)
        .checked_mul(price as u128)
        .ok_or(error!(VaultError::MathOverflow))?
        .checked_div(10u128.pow(asset_decimals as u32))
        .ok_or(error!(VaultError::DivisionByZero))?;
    u64::try_from(value).map_err(|_| error!(VaultError::MathOverflow))
}

/// Compute total portfolio value across all basket assets.
pub fn total_portfolio_value(
    balances: &[u64],
    prices: &[u64],
    decimals: &[u8],
) -> Result<u64> {
    let mut total: u128 = 0;
    for i in 0..balances.len() {
        let value = (balances[i] as u128)
            .checked_mul(prices[i] as u128)
            .ok_or(error!(VaultError::MathOverflow))?
            .checked_div(10u128.pow(decimals[i] as u32))
            .ok_or(error!(VaultError::DivisionByZero))?;
        total = total
            .checked_add(value)
            .ok_or(error!(VaultError::MathOverflow))?;
    }
    u64::try_from(total).map_err(|_| error!(VaultError::MathOverflow))
}
