# SVS-8: Multi-Asset Basket Vault

## Overview

SVS-8 implements a multi-asset tokenized vault where a single share token represents proportional ownership of a basket of up to 8 underlying SPL tokens. Portfolio valuation uses oracle price feeds to determine each asset's value in a common base unit (e.g., USD with 6 decimals).

**Use cases:** Index funds, treasury management, diversified yield strategies, and any product where a single tokenized position represents exposure to multiple assets.

## Architecture

```
                    ┌─────────────┐
                    │ Shares Mint │  (Token-2022)
                    │   (LP Token)│
                    └──────┬──────┘
                           │
                    ┌──────┴──────┐
                    │ MultiAsset  │  PDA: ["multi_vault", vault_id]
                    │   Vault     │
                    └──────┬──────┘
                           │
            ┌──────────────┼──────────────┐
            │              │              │
     ┌──────┴──────┐ ┌────┴────┐  ┌──────┴──────┐
     │ AssetEntry 0│ │AssetE. 1│  │ AssetEntry N│  PDA: ["asset_entry", vault, mint]
     │ USDC (40%)  │ │SOL (30%)│  │ BTC  (30%) │
     └──────┬──────┘ └────┬────┘  └──────┬──────┘
            │              │              │
     ┌──────┴──────┐ ┌────┴────┐  ┌──────┴──────┐
     │ Asset Vault │ │  Asset  │  │ Asset Vault │  (PDA-owned token accounts)
     │  (tokens)   │ │  Vault  │  │  (tokens)   │
     └─────────────┘ └─────────┘  └─────────────┘
```

## Key Differences from SVS-1

| Aspect | SVS-1 | SVS-8 |
|--------|-------|-------|
| Underlying assets | Single SPL token | N SPL tokens (max 8) |
| Asset vaults | One PDA-owned account | N PDA-owned accounts |
| Total assets | Single u64 balance | Weighted sum via oracles |
| Deposit | Transfer one token | Single or proportional |
| Share price | `assets / shares` | `portfolio_value / shares` |
| Oracle | Not needed | Required for all assets |

## Instructions

| # | Instruction | Signer | Description |
|---|------------|--------|-------------|
| 1 | `initialize` | Authority | Create vault PDA and shares mint |
| 2 | `add_asset` | Authority | Add asset to basket with weight |
| 3 | `remove_asset` | Authority | Remove asset (zero balance only) |
| 4 | `update_weights` | Authority | Set new target weights (must sum to 10000) |
| 5 | `deposit_single` | User | Deposit one asset, receive shares |
| 6 | `deposit_proportional` | User | Deposit all assets in weight proportions |
| 7 | `redeem_single` | User | Redeem shares for one asset |
| 8 | `redeem_proportional` | User | Redeem shares for proportional basket |
| 9 | `pause/unpause` | Authority | Emergency controls |
| 10 | `transfer_authority` | Authority | Transfer admin |
| 11 | `initialize_oracle` | Authority | Create price oracle (testing) |
| 12 | `update_oracle` | Authority | Update oracle price |

## Oracle System

Each `AssetEntry` references an oracle account that provides the asset's price in base units. The vault reads ALL oracle prices at deposit/redeem time to compute total portfolio value.

**Staleness behavior:**
- All oracles fresh → operations proceed
- Any oracle stale (>5 min) → ALL operations blocked
- Invalid oracle → transaction fails

For testing, SVS-8 includes built-in `OraclePrice` accounts. In production, integrate Pyth or Switchboard directly.

## Weight Invariant

**Critical invariant:** `sum(target_weight_bps) == 10,000` (100%)

Enforced on `add_asset`, `remove_asset`, and `update_weights`. The vault cannot enter a state where weights don't sum to 100%.

## Remaining Accounts Pattern

Due to variable asset count, per-asset data is passed via `remaining_accounts`:

- `deposit_single`: `[AssetEntry, asset_vault, oracle] × num_assets`
- `deposit_proportional`: `[AssetEntry, asset_vault, oracle, user_token, asset_mint] × num_assets`
- `redeem_proportional`: `[AssetEntry, asset_vault, user_token, asset_mint] × num_assets`

## Security Properties

- **Inflation attack protection** via virtual offset (same as SVS-1)
- **Slippage protection** on all deposit/redeem operations
- **Oracle validation** prevents stale/manipulated price data
- **Checked arithmetic** throughout (no unchecked operations)
- **Stored PDA bumps** (no recalculation)
- **Rounding favors vault** (floor on deposits, floor on redemptions)

## Compute Unit Estimates

| Instruction | CU | Notes |
|-------------|------|-------|
| initialize | ~25K | Create vault + shares mint |
| add_asset | ~35K | Create AssetEntry + vault |
| deposit_single | ~50K | All oracles + transfer + mint |
| deposit_proportional | ~80K | N transfers + N reads |
| redeem_proportional | ~90K | N transfers + burn |

## Module Compatibility

SVS-8 is compatible with all SVS modules:
- **svs-fees**: Computed on base-unit value
- **svs-caps**: Cap on total_portfolio_value
- **svs-locks**: Share-based (identical)
- **svs-rewards**: Per-share distribution
- **svs-access**: Identity checks

## See Also

- [SVS-1](./SVS-1.md) — Single-asset vault reference
- [specs-SVS08.md](./specs-SVS08.md) — Detailed specification
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Cross-variant design
