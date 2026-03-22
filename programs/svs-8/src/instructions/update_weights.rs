//! Update target weights for all basket assets. Weights must sum to 10000 bps.

use anchor_lang::prelude::*;

use crate::{
    constants::WEIGHT_BPS_DENOMINATOR,
    error::VaultError,
    events::WeightsUpdated,
    state::{AssetEntry, MultiAssetVault},
};

#[derive(Accounts)]
pub struct UpdateWeights<'info> {
    pub authority: Signer<'info>,

    #[account(
        has_one = authority @ VaultError::Unauthorized,
    )]
    pub vault: Account<'info, MultiAssetVault>,

    // remaining_accounts: all AssetEntry accounts (mut) in basket
}

pub fn handler(ctx: Context<UpdateWeights>, new_weights: Vec<u16>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let num_assets = vault.num_assets as usize;

    require!(
        new_weights.len() == num_assets,
        VaultError::RemainingAccountsMismatch
    );
    require!(
        ctx.remaining_accounts.len() == num_assets,
        VaultError::RemainingAccountsMismatch
    );

    // Validate weights sum to 10000
    let total_weight: u16 = new_weights
        .iter()
        .try_fold(0u16, |acc, &w| acc.checked_add(w))
        .ok_or(VaultError::MathOverflow)?;
    require!(
        total_weight == WEIGHT_BPS_DENOMINATOR,
        VaultError::InvalidWeight
    );

    // Update each AssetEntry
    for (i, account_info) in ctx.remaining_accounts.iter().enumerate() {
        require!(
            account_info.is_writable,
            VaultError::RemainingAccountsMismatch
        );
        require!(
            *account_info.owner == crate::ID,
            VaultError::RemainingAccountsMismatch
        );

        let mut data = account_info.try_borrow_mut_data()?;
        let mut entry = AssetEntry::try_deserialize(&mut &data[..])
            .map_err(|_| error!(VaultError::RemainingAccountsMismatch))?;
        require!(entry.vault == vault.key(), VaultError::AssetNotFound);

        entry.target_weight_bps = new_weights[i];

        // Serialize back (skip discriminator)
        let dst = &mut data[8..];
        let serialized = entry
            .try_to_vec()
            .map_err(|_| error!(VaultError::MathOverflow))?;
        dst[..serialized.len()].copy_from_slice(&serialized);
    }

    emit!(WeightsUpdated {
        vault: vault.key(),
    });

    Ok(())
}
