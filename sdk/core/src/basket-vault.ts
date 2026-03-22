/**
 * Basket Vault Module (SVS-8)
 *
 * Multi-asset basket vault holding N underlying SPL tokens with oracle-based
 * portfolio valuation. A single shares mint represents proportional ownership
 * of the entire portfolio. Deposits and redemptions can be made in any accepted
 * asset or proportionally across all assets.
 *
 * PDA seeds:
 * - Vault: ["multi_vault", vault_id (u64 LE)]
 * - Shares Mint: ["shares", vault_pubkey]
 * - Asset Entry: ["asset_entry", vault_pubkey, asset_mint]
 * - Oracle: ["oracle", vault_pubkey, asset_mint]
 *
 * @example
 * ```ts
 * import { BasketVault } from "@stbr/solana-vault";
 *
 * const vault = await BasketVault.load(program, vaultId);
 * await vault.depositSingle(user, assetMint, amount, minSharesOut);
 * await vault.redeemProportional(user, shares);
 * ```
 */

import { BN, Program, AnchorProvider } from "@coral-xyz/anchor";
import {
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountIdempotentInstruction,
} from "@solana/spl-token";

const MULTI_VAULT_SEED = Buffer.from("multi_vault");
const SHARES_MINT_SEED = Buffer.from("shares");
const ASSET_ENTRY_SEED = Buffer.from("asset_entry");
const ORACLE_SEED = Buffer.from("oracle");

export interface BasketAsset {
  mint: PublicKey;
  entry: PublicKey;
  oracle: PublicKey;
  vaultTokenAccount: PublicKey;
  decimals: number;
  targetWeightBps: number;
  price: BN;
}

export interface BasketVaultState {
  authority: PublicKey;
  sharesMint: PublicKey;
  vaultId: BN;
  numAssets: number;
  decimalsOffset: number;
  baseDecimals: number;
  totalShares: BN;
  paused: boolean;
  bump: number;
}

/**
 * Derive the multi-asset vault PDA address.
 */
export function getMultiVaultAddress(
  programId: PublicKey,
  vaultId: BN | number,
): [PublicKey, number] {
  const id = typeof vaultId === "number" ? new BN(vaultId) : vaultId;
  return PublicKey.findProgramAddressSync(
    [MULTI_VAULT_SEED, id.toArrayLike(Buffer, "le", 8)],
    programId,
  );
}

/**
 * Derive the shares mint PDA for a basket vault.
 */
export function getBasketSharesMintAddress(
  programId: PublicKey,
  vault: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SHARES_MINT_SEED, vault.toBuffer()],
    programId,
  );
}

/**
 * Derive the asset entry PDA for a given asset in the basket.
 */
export function getAssetEntryAddress(
  programId: PublicKey,
  vault: PublicKey,
  assetMint: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [ASSET_ENTRY_SEED, vault.toBuffer(), assetMint.toBuffer()],
    programId,
  );
}

/**
 * Derive the oracle price PDA for a given asset.
 */
export function getOracleAddress(
  programId: PublicKey,
  vault: PublicKey,
  assetMint: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [ORACLE_SEED, vault.toBuffer(), assetMint.toBuffer()],
    programId,
  );
}

/**
 * Derive all PDAs for a basket vault at once.
 */
export function deriveBasketVaultAddresses(
  programId: PublicKey,
  vaultId: BN | number,
) {
  const [vault, vaultBump] = getMultiVaultAddress(programId, vaultId);
  const [sharesMint, sharesMintBump] = getBasketSharesMintAddress(
    programId,
    vault,
  );
  return { vault, vaultBump, sharesMint, sharesMintBump };
}

/**
 * Derive all PDAs for an asset within a basket vault.
 */
export function deriveAssetAddresses(
  programId: PublicKey,
  vault: PublicKey,
  assetMint: PublicKey,
  assetTokenProgram: PublicKey,
) {
  const [entry, entryBump] = getAssetEntryAddress(programId, vault, assetMint);
  const [oracle, oracleBump] = getOracleAddress(programId, vault, assetMint);
  const assetVault = getAssociatedTokenAddressSync(
    assetMint,
    vault,
    true,
    assetTokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  return { entry, entryBump, oracle, oracleBump, assetVault };
}

/**
 * Build remaining accounts for view/deposit_single/redeem_single.
 * Pattern: [AssetEntry, asset_vault, oracle] × num_assets
 */
export function buildViewRemainingAccounts(
  assets: BasketAsset[],
): { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] {
  const accounts: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [];
  for (const asset of assets) {
    accounts.push(
      { pubkey: asset.entry, isSigner: false, isWritable: false },
      { pubkey: asset.vaultTokenAccount, isSigner: false, isWritable: false },
      { pubkey: asset.oracle, isSigner: false, isWritable: false },
    );
  }
  return accounts;
}

/**
 * Build remaining accounts for deposit_single.
 * Pattern: [AssetEntry, asset_vault, oracle] × num_assets (all assets for valuation)
 * The target asset is identified by the named accounts in the instruction.
 */
export function buildDepositSingleRemainingAccounts(
  assets: BasketAsset[],
): { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] {
  return buildViewRemainingAccounts(assets);
}

/**
 * Build remaining accounts for deposit_proportional.
 * Pattern: [AssetEntry, asset_vault(mut), user_token(mut), oracle, mint] × num_assets
 */
export function buildDepositProportionalRemainingAccounts(
  assets: BasketAsset[],
  userTokenAccounts: PublicKey[],
  assetMints: PublicKey[],
): { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] {
  const accounts: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [];
  for (let i = 0; i < assets.length; i++) {
    accounts.push(
      { pubkey: assets[i].entry, isSigner: false, isWritable: false },
      { pubkey: assets[i].vaultTokenAccount, isSigner: false, isWritable: true },
      { pubkey: userTokenAccounts[i], isSigner: false, isWritable: true },
      { pubkey: assets[i].oracle, isSigner: false, isWritable: false },
      { pubkey: assetMints[i], isSigner: false, isWritable: false },
    );
  }
  return accounts;
}

/**
 * Build remaining accounts for redeem_proportional.
 * Pattern: [AssetEntry, asset_vault(mut), user_token(mut), mint] × num_assets
 */
export function buildRedeemProportionalRemainingAccounts(
  assets: BasketAsset[],
  userTokenAccounts: PublicKey[],
  assetMints: PublicKey[],
): { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] {
  const accounts: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [];
  for (let i = 0; i < assets.length; i++) {
    accounts.push(
      { pubkey: assets[i].entry, isSigner: false, isWritable: false },
      { pubkey: assets[i].vaultTokenAccount, isSigner: false, isWritable: true },
      { pubkey: userTokenAccounts[i], isSigner: false, isWritable: true },
      { pubkey: assetMints[i], isSigner: false, isWritable: false },
    );
  }
  return accounts;
}

/**
 * Build remaining accounts for update_weights.
 * Pattern: [AssetEntry(mut)] × num_assets
 */
export function buildUpdateWeightsRemainingAccounts(
  assets: BasketAsset[],
): { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] {
  return assets.map((asset) => ({
    pubkey: asset.entry,
    isSigner: false,
    isWritable: true,
  }));
}

/**
 * Create an instruction to create a shares ATA for a user (idempotent).
 */
export function createBasketSharesAtaIx(
  payer: PublicKey,
  owner: PublicKey,
  sharesMint: PublicKey,
): TransactionInstruction {
  return createAssociatedTokenAccountIdempotentInstruction(
    payer,
    getAssociatedTokenAddressSync(
      sharesMint,
      owner,
      false,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    ),
    owner,
    sharesMint,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
}
