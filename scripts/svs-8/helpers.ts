/**
 * SVS-8 helpers — re-exports shared utilities with SVS-8 types.
 * SVS-8 uses "multi_vault" seed (not "vault") for the vault PDA.
 */

import { Program, BN } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, TransactionInstruction } from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import * as anchor from "@coral-xyz/anchor";
import { Svs8 } from "../../target/types/svs_8";
import {
  setupTest as genericSetupTest,
  type SetupResult as GenericSetupResult,
} from "../shared/common-helpers";

export {
  RPC_URL,
  ASSET_DECIMALS,
  SHARE_DECIMALS,
  loadKeypair,
  explorerUrl,
  accountUrl,
  fundAccount,
  fundAccounts,
} from "../shared/common-helpers";

/** SVS-8 vault PDA uses "multi_vault" seed */
export function getVaultPDA(programId: PublicKey, vaultId: BN): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("multi_vault"), vaultId.toArrayLike(Buffer, "le", 8)],
    programId,
  );
}

/** Derive shares mint PDA from vault */
export function getSharesMintPDA(programId: PublicKey, vault: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("shares"), vault.toBuffer()],
    programId,
  );
}

/** Derive asset entry PDA */
export function getAssetEntryPDA(
  programId: PublicKey,
  vault: PublicKey,
  assetMint: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("asset_entry"), vault.toBuffer(), assetMint.toBuffer()],
    programId,
  );
}

/** Derive oracle price PDA */
export function getOraclePDA(
  programId: PublicKey,
  vault: PublicKey,
  assetMint: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("oracle"), vault.toBuffer(), assetMint.toBuffer()],
    programId,
  );
}

export interface SetupResult {
  connection: Connection;
  payer: Keypair;
  provider: anchor.AnchorProvider;
  program: Program<Svs8>;
  programId: PublicKey;
}

export async function setupTest(testName: string): Promise<SetupResult> {
  return genericSetupTest<Svs8>(testName, "svs_8" as any);
}

/** Create ATA for shares token (idempotent) */
export function createSharesAtaIx(
  payer: PublicKey,
  owner: PublicKey,
  sharesMint: PublicKey,
): TransactionInstruction {
  return createAssociatedTokenAccountIdempotentInstruction(
    payer,
    getAssociatedTokenAddressSync(sharesMint, owner, false, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID),
    owner,
    sharesMint,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
}

/** Get asset vault ATA (vault-owned token account for an asset) */
export function getAssetVaultATA(
  assetMint: PublicKey,
  vault: PublicKey,
  tokenProgram: PublicKey,
): PublicKey {
  return getAssociatedTokenAddressSync(
    assetMint,
    vault,
    true,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
}
