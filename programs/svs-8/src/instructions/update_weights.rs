//! Update weights instruction: rebalance target allocations (must sum to 10000).

use anchor_lang::prelude::*;

use crate::{
    constants::{ASSET_ENTRY_SEED, BPS_DENOMINATOR},
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
    // remaining_accounts: [AssetEntry (mut)] × num_assets
    // Each AssetEntry is mutable to update target_weight_bps
}

pub fn handler(ctx: Context<UpdateWeights>, new_weights: Vec<u16>) -> Result<()> {
    let vault = &ctx.accounts.vault;

    // Validate correct number of weights provided
    require!(
        new_weights.len() == vault.num_assets as usize,
        VaultError::InvalidRemainingAccounts
    );
    require!(
        ctx.remaining_accounts.len() == vault.num_assets as usize,
        VaultError::InvalidRemainingAccounts
    );

    // Validate weights sum to 10000
    let total: u16 = new_weights
        .iter()
        .try_fold(0u16, |acc, &w| acc.checked_add(w))
        .ok_or(VaultError::MathOverflow)?;
    require!(total == BPS_DENOMINATOR, VaultError::InvalidWeight);

    // Validate all weights are positive
    for &w in &new_weights {
        require!(w > 0, VaultError::InvalidWeight);
    }

    // Update each AssetEntry's target_weight_bps
    let vault_key = vault.key();
    for (i, account_info) in ctx.remaining_accounts.iter().enumerate() {
        // Validate the account is a valid AssetEntry for this vault
        let mut data = account_info.try_borrow_mut_data()?;
        let mut entry = AssetEntry::try_deserialize(&mut &data[..])?;

        require!(entry.vault == vault_key, VaultError::AssetNotFound);

        // Verify PDA derivation
        let (expected_pda, _) = Pubkey::find_program_address(
            &[ASSET_ENTRY_SEED, vault_key.as_ref(), entry.asset_mint.as_ref()],
            ctx.program_id,
        );
        require!(
            account_info.key() == expected_pda,
            VaultError::AssetNotFound
        );

        entry.target_weight_bps = new_weights[i];
        entry.try_serialize(&mut &mut data[..])?;
    }

    emit!(WeightsUpdated {
        vault: vault_key,
    });

    Ok(())
}
