# SVS-7: Native SOL Vault

Tokenized vault that accepts native SOL deposits, auto-wraps them as wSOL internally, and exposes a standard ERC-4626-style vault interface. On withdrawal, wSOL is automatically unwrapped back to native SOL.

## Purpose

SOL is Solana's native token, but most DeFi protocols require SPL Token accounts. SVS-7 bridges this gap by providing a vault that:

- Accepts plain SOL transfers (no wSOL wrapping needed by the user)
- Internally manages wSOL for composability with other vault modules
- Returns native SOL on withdrawal (no manual unwrapping)
- Exposes the standard SVS share-based vault interface

This allows downstream protocols to integrate SOL deposits through the same vault interface used for any other SPL token.

## Architecture

```
User (native SOL)
  |
  | deposit_sol (system_program::transfer)
  v
+-------------------+     +------------------+     +------------------+
| NativeSolVault    |---->| wSOL Vault (PDA) |     | Shares Mint      |
| PDA               |     | SPL Token Acct   |     | Token-2022       |
|                   |     | owner: vault PDA |     | authority: vault |
| - authority       |     +------------------+     +------------------+
| - total_sol       |           |                         |
| - vault_id        |           | sync_native             | mint_to / burn
| - bumps (3)       |           v                         v
+-------------------+     wSOL balance synced       User Shares ATA
  |
  | withdraw_sol
  v
User (native SOL) <-- close_account(user_wsol_ata)
```

### PDA Derivation

| Account | Seeds | Program |
|---------|-------|---------|
| Vault | `["native_sol_vault", vault_id_le_bytes]` | SVS-7 |
| Shares Mint | `["shares", vault_pubkey]` | Token-2022 |
| wSOL Vault | `["wsol_vault", vault_pubkey]` | SPL Token |

## How SOL Wrapping Works

### Deposit Flow

1. User sends native SOL via `system_program::transfer` to the wSOL vault PDA
2. `sync_native` instruction updates the wSOL token account balance to match its lamport balance
3. Shares are minted proportional to the deposited SOL amount (floor rounding)

### Withdrawal Flow

1. User's shares are burned via Token-2022 `burn`
2. wSOL is transferred from vault to the user's ephemeral wSOL ATA via `transfer_checked`
3. The user's wSOL ATA is closed via `close_account`, sending all lamports (wSOL value + rent) back to the user as native SOL

The ephemeral wSOL ATA is created (if needed) and closed within the same transaction, so the user never needs to manage wSOL directly.

## Instructions

| Instruction | Description |
|-------------|-------------|
| `initialize` | Create vault, shares mint, and wSOL vault account |
| `deposit_sol` | Accept native SOL, wrap to wSOL, mint shares |
| `withdraw_sol` | Burn shares, unwrap wSOL, return native SOL |
| `pause` / `unpause` | Emergency circuit breaker |
| `transfer_authority` | Transfer admin control |
| `preview_deposit` | Query: shares for given lamports |
| `preview_withdraw` | Query: lamports for given shares |
| `convert_to_shares` | Query: asset-to-share conversion |
| `convert_to_assets` | Query: share-to-asset conversion |
| `total_assets` | Query: total SOL in vault |
| `max_deposit` / `max_mint` | Query: deposit/mint limits |
| `max_withdraw` / `max_redeem` | Query: per-user withdrawal limits |

## Share Conversion Math

Uses the same `svs-math` library as all other SVS variants:

- **Virtual offset**: `decimals_offset = 9 - 9 = 0` for SOL (same decimals as shares)
- **Deposit**: `shares = assets * (total_shares + 10^offset) / (total_assets + 10^offset)` (floor)
- **Withdraw**: `assets = shares * (total_assets + 10^offset) / (total_shares + 10^offset)` (floor)
- Rounding always favors the vault to protect existing shareholders

## Security Considerations

### Inflation Attack Protection
Virtual shares/assets offset prevents the classic ERC-4626 inflation attack where an attacker front-runs the first depositor with a large direct token transfer.

### Checked Arithmetic
All arithmetic operations use `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, and the `svs-math` safe operations. No `unwrap()` in program code.

### PDA Bump Storage
All three PDA bumps (vault, shares mint, wSOL vault) are stored canonically at initialization time to save ~1500 CU per access and prevent bump manipulation.

### Rent-Exempt Safety
The wSOL vault token account maintains rent-exempt minimum at all times. Available balance for withdrawal is `lamports - rent_exempt_minimum`.

### Slippage Protection
Both `deposit_sol` and `withdraw_sol` accept slippage parameters (`min_shares_out` and `min_sol_out`) to protect against front-running.

### Pause Mechanism
Authority can pause all deposit/withdrawal operations in case of emergency. View functions remain operational.

### Account Reload After CPI
wSOL vault account is reloaded after `sync_native` and `transfer_checked` CPIs to ensure accurate balance reads.

## Events

| Event | Emitted On |
|-------|-----------|
| `VaultInitialized` | Vault creation |
| `SolDeposited` | SOL deposit + share mint |
| `SolWithdrawn` | Share burn + SOL withdrawal |
| `VaultStatusChanged` | Pause/unpause |
| `AuthorityTransferred` | Authority change |

## Build

```bash
anchor build -p svs-7
```

## License

Apache-2.0
