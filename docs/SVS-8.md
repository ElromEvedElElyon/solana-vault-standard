# SVS-8: Multi-Asset Basket Vault

## Overview

SVS-8 holds a weighted basket of multiple underlying SPL tokens. A single shares mint (Token-2022) represents proportional ownership of the entire portfolio. Users deposit and redeem via single-asset or proportional operations. Share pricing uses oracle-weighted portfolio valuation in a common base unit (e.g., USD with 6 decimals).

Targets index funds, treasury management, diversified yield strategies, and any product where a single tokenized position represents exposure to multiple assets.

## How It Differs from SVS-1

| Aspect | SVS-1 | SVS-8 |
|--------|-------|-------|
| Underlying assets | Single SPL token | Up to 16 SPL tokens (configurable basket) |
| Asset storage | One ATA (PDA-owned) | N ATAs (one per asset, PDA-owned) |
| `total_assets` | Single `u64` balance | Weighted sum via oracle prices (base units) |
| Deposit | Transfer one token | Single-asset or proportional basket deposit |
| Redeem | Receive one token | Single-asset or proportional basket redeem |
| Share price | `total_assets / total_shares` | `total_portfolio_value / total_shares` |
| Oracle dependency | None | Required for all basket assets |

## Account Structure

### PDA Derivation

| Account | Seeds | Owner |
|---------|-------|-------|
| MultiAssetVault | `["multi_vault", vault_id.to_le_bytes()]` | Program |
| Shares Mint | `["shares", vault_pubkey]` | Token-2022 |
| AssetEntry | `["asset_entry", vault_pubkey, asset_mint]` | Program |
| OraclePrice | `["oracle", vault_pubkey, asset_mint]` | Program |
| Asset Vault | ATA of vault PDA for each asset mint | Vault PDA |

### State Structs

**MultiAssetVault — 157 bytes:**
```rust
pub struct MultiAssetVault {
    pub authority: Pubkey,       // 32
    pub shares_mint: Pubkey,     // 32
    pub total_shares: u64,       // 8
    pub decimals_offset: u8,     // 1
    pub bump: u8,                // 1
    pub paused: bool,            // 1
    pub vault_id: u64,           // 8
    pub num_assets: u8,          // 1
    pub base_decimals: u8,       // 1
    pub _reserved: [u8; 64],     // 64
}
```

**AssetEntry — 133 bytes:**
```rust
pub struct AssetEntry {
    pub vault: Pubkey,           // 32
    pub asset_mint: Pubkey,      // 32
    pub asset_vault: Pubkey,     // 32
    pub oracle: Pubkey,          // 32
    pub target_weight_bps: u16,  // 2
    pub asset_decimals: u8,      // 1
    pub index: u8,               // 1
    pub bump: u8,                // 1
}
```

**OraclePrice — 81 bytes:**
```rust
pub struct OraclePrice {
    pub vault: Pubkey,           // 32
    pub asset_mint: Pubkey,      // 32
    pub price: u64,              // 8
    pub updated_at: i64,         // 8
    pub bump: u8,                // 1
}
```

## Instruction Set

| # | Instruction | Signer | Description |
|---|------------|--------|-------------|
| 1 | `initialize` | Authority | Create MultiAssetVault PDA and Token-2022 shares mint |
| 2 | `add_asset` | Authority | Add AssetEntry + OraclePrice + ATA to basket |
| 3 | `remove_asset` | Authority | Remove asset (must have zero balance) |
| 4 | `update_weights` | Authority | Set new target weights (must sum to 10000 bps) |
| 5 | `set_price` | Authority | Update oracle price for an asset (devnet/testing) |
| 6 | `deposit_single` | User | Deposit one asset, mint shares based on oracle value |
| 7 | `deposit_proportional` | User | Deposit all assets in target weight proportions |
| 8 | `redeem_single` | User | Redeem shares for one specific asset |
| 9 | `redeem_proportional` | User | Redeem shares for proportional basket |
| 10 | `pause` | Authority | Halt all deposits/redeems |
| 11 | `unpause` | Authority | Resume operations |
| 12 | `transfer_authority` | Authority | Transfer admin rights |
| 13 | `total_portfolio_value` | Any | View: total basket value in base units |
| 14 | `preview_deposit` | Any | View: shares for a given deposit value |
| 15 | `preview_redeem` | Any | View: assets received for given shares |

## Pricing Model

Each asset's value is converted to a common base unit using its oracle price:

```
asset_value = balance * price / 10^asset_decimals
total_portfolio_value = sum(asset_value for all assets)
shares = deposit_value * (total_shares + offset) / (total_value + 1)  [floor]
assets = shares * (total_value + 1) / (total_shares + offset)         [floor]
```

Virtual offset: `10^(9 - base_decimals)` — same inflation attack protection as SVS-1.

Rounding always favors the vault (floor on deposit shares, floor on redeem assets).

## Remaining Accounts Pattern

Variable-length per-asset data is passed via `remaining_accounts`:

| Operation | Pattern per asset | Total accounts |
|-----------|-------------------|----------------|
| View / DepositSingle | `[AssetEntry, vault_ata, OraclePrice]` | 3 × num_assets |
| Proportional ops | `[AssetEntry, vault_ata, OraclePrice, user_ata]` | 4 × num_assets |

## Weight Invariant

`sum(target_weight_bps for all AssetEntry) == 10_000`

Enforced on `add_asset`, `remove_asset`, and `update_weights`. The vault cannot enter a state where weights don't sum to 100%.

## Security Properties

- Checked arithmetic everywhere (`checked_add`/`checked_sub`/`checked_mul`/`checked_div`) — zero `unwrap()` in program code
- Virtual shares offset protects against inflation attacks
- Rounding favors the vault on all entry/exit operations
- PDA bumps stored on-chain (canonical bump, never recalculated)
- Authority-only admin operations (add/remove asset, update weights, pause/unpause, set price)
- Pause mechanism halts deposits/redeems during emergencies
- Weight validation ensures total weights sum to 10,000 bps (100%)
- MAX_ASSETS cap (16) prevents unbounded account growth
- Box<Account> pattern for large instruction contexts avoids SBF stack overflow
- Slippage protection on deposit (min_shares_out) and redeem (min_assets_out)

## Module Compatibility

| Module | Compatible | Notes |
|--------|-----------|-------|
| svs-fees | Yes | Fees computed on base-unit value of deposit/redemption |
| svs-caps | Yes | Global cap on total_portfolio_value, per-user cap on cumulative deposited value |
| svs-locks | Yes | Works identically (share-based) |
| svs-access | Yes | Identity-based checks, variant-agnostic |
| svs-rewards | Yes | Rewards distributed per-share regardless of basket composition |

## Limitations

- **Max 16 assets per basket.** Practical limit from account size and compute budget.
- **Oracle dependency.** Every financial operation requires fresh prices for ALL basket assets. A single stale oracle blocks the entire vault.
- **No atomic rebalancing.** Not implemented in MVP. Authority can manually rebalance via external swaps.
- **Authority-managed oracles.** Current implementation uses `set_price` for testing. Production deployments should integrate Pyth or Switchboard.

## See Also

- [specs-SVS08.md](./specs-SVS08.md) — Original specification
- [SVS-1.md](./SVS-1.md) — Base single-asset vault
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Cross-variant design
