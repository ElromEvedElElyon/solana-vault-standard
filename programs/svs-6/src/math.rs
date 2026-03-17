//! Math module - re-exports from svs-math with Anchor error conversion,
//! plus streaming yield interpolation for SVS-6.

use anchor_lang::prelude::*;

use crate::error::VaultError;
use crate::state::ConfidentialStreamVault;

pub use svs_math::Rounding;

/// Convert assets to shares with virtual offset protection against inflation attacks.
pub fn convert_to_shares(
    assets: u64,
    total_assets: u64,
    total_shares: u64,
    decimals_offset: u8,
    rounding: Rounding,
) -> Result<u64> {
    svs_math::convert_to_shares(
        assets,
        total_assets,
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
    total_assets: u64,
    total_shares: u64,
    decimals_offset: u8,
    rounding: Rounding,
) -> Result<u64> {
    svs_math::convert_to_assets(
        shares,
        total_assets,
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

/// Compute effective total_assets at a given timestamp.
///
/// This replaces direct reads of asset_vault.amount or vault.total_assets.
/// The effective total assets is the base_assets plus any accrued streaming yield
/// interpolated linearly between stream_start and stream_end.
pub fn effective_total_assets(vault: &ConfidentialStreamVault, now: i64) -> Result<u64> {
    if now >= vault.stream_end || vault.stream_start >= vault.stream_end {
        // Stream complete or no active stream
        return vault
            .base_assets
            .checked_add(vault.stream_amount)
            .ok_or_else(|| error!(VaultError::MathOverflow));
    }
    if now <= vault.stream_start {
        return Ok(vault.base_assets);
    }

    let elapsed = (now - vault.stream_start) as u64;
    let duration = (vault.stream_end - vault.stream_start) as u64;
    let accrued = mul_div(vault.stream_amount, elapsed, duration, Rounding::Floor)?;

    vault
        .base_assets
        .checked_add(accrued)
        .ok_or_else(|| error!(VaultError::MathOverflow))
}
