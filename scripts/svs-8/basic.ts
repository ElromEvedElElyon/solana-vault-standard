/**
 * SVS-8 Basic Test Script
 *
 * Tests core multi-asset basket vault functionality:
 * - Initialize vault
 * - Add assets with target weights
 * - Set oracle prices
 * - Deposit single asset
 * - Redeem single asset
 * - Pause/unpause
 *
 * Run: npx ts-node scripts/svs-8/basic.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { BN } from "@coral-xyz/anchor";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAccount,
  getAssociatedTokenAddressSync,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, SystemProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import {
  setupTest,
  getVaultPDA,
  getSharesMintPDA,
  getAssetEntryPDA,
  getOraclePDA,
  getAssetVaultATA,
  createSharesAtaIx,
  explorerUrl,
  accountUrl,
} from "./helpers";

const INITIAL_MINT_AMOUNT = 1_000_000;
const DEPOSIT_AMOUNT = 100_000;

async function main() {
  const { connection, payer, program, programId } = await setupTest("Basic Functionality");

  // Step 1: Create two test tokens (mock SOL and mock USDC)
  console.log("\n" + "-".repeat(70));
  console.log("Step 1: Creating test tokens");
  console.log("-".repeat(70));

  const mintA = await createMint(
    connection, payer, payer.publicKey, null, 9,
    Keypair.generate(), undefined, TOKEN_PROGRAM_ID,
  );
  console.log(`  Mint A (9 dec, SOL-like): ${mintA.toBase58()}`);

  const mintB = await createMint(
    connection, payer, payer.publicKey, null, 6,
    Keypair.generate(), undefined, TOKEN_PROGRAM_ID,
  );
  console.log(`  Mint B (6 dec, USDC-like): ${mintB.toBase58()}`);

  // Step 2: Mint tokens to user
  console.log("\n" + "-".repeat(70));
  console.log("Step 2: Minting tokens to user");
  console.log("-".repeat(70));

  const userAtaA = await getOrCreateAssociatedTokenAccount(
    connection, payer, mintA, payer.publicKey, false, undefined, undefined, TOKEN_PROGRAM_ID,
  );
  const userAtaB = await getOrCreateAssociatedTokenAccount(
    connection, payer, mintB, payer.publicKey, false, undefined, undefined, TOKEN_PROGRAM_ID,
  );

  await mintTo(connection, payer, mintA, userAtaA.address, payer.publicKey,
    INITIAL_MINT_AMOUNT * 10 ** 9, [], undefined, TOKEN_PROGRAM_ID);
  await mintTo(connection, payer, mintB, userAtaB.address, payer.publicKey,
    INITIAL_MINT_AMOUNT * 10 ** 6, [], undefined, TOKEN_PROGRAM_ID);

  console.log(`  Minted ${INITIAL_MINT_AMOUNT.toLocaleString()} of each token`);

  // Step 3: Derive PDAs
  console.log("\n" + "-".repeat(70));
  console.log("Step 3: Deriving PDAs");
  console.log("-".repeat(70));

  const vaultId = new BN(Date.now());
  const [vault] = getVaultPDA(programId, vaultId);
  const [sharesMint] = getSharesMintPDA(programId, vault);
  const [entryA] = getAssetEntryPDA(programId, vault, mintA);
  const [entryB] = getAssetEntryPDA(programId, vault, mintB);
  const [oracleA] = getOraclePDA(programId, vault, mintA);
  const [oracleB] = getOraclePDA(programId, vault, mintB);

  const assetVaultA = getAssetVaultATA(mintA, vault, TOKEN_PROGRAM_ID);
  const assetVaultB = getAssetVaultATA(mintB, vault, TOKEN_PROGRAM_ID);

  console.log(`  Vault: ${vault.toBase58()}`);
  console.log(`  Shares Mint: ${sharesMint.toBase58()}`);
  console.log(`  Entry A: ${entryA.toBase58()}`);
  console.log(`  Entry B: ${entryB.toBase58()}`);

  // Step 4: Initialize vault
  console.log("\n" + "-".repeat(70));
  console.log("Step 4: Initializing vault");
  console.log("-".repeat(70));

  const initTx = await program.methods
    .initialize(vaultId, 6) // base_decimals = 6 (USD)
    .accountsStrict({
      authority: payer.publicKey,
      vault,
      sharesMint,
      token2022Program: TOKEN_2022_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: SYSVAR_RENT_PUBKEY,
    })
    .rpc();

  console.log(`  TX: ${explorerUrl(initTx)}`);

  // Step 5: Add Asset A (60% weight, price = $100)
  console.log("\n" + "-".repeat(70));
  console.log("Step 5: Adding Asset A (60% weight, $100)");
  console.log("-".repeat(70));

  const addATx = await program.methods
    .addAsset(6000, new BN(100_000_000)) // 60% weight, price $100 (6 dec)
    .accountsStrict({
      authority: payer.publicKey,
      vault,
      assetMint: mintA,
      assetEntry: entryA,
      oracle: oracleA,
      assetVault: assetVaultA,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: SYSVAR_RENT_PUBKEY,
    })
    .remainingAccounts([])
    .rpc();

  console.log(`  TX: ${explorerUrl(addATx)}`);

  // Step 6: Add Asset B (40% weight, price = $1)
  console.log("\n" + "-".repeat(70));
  console.log("Step 6: Adding Asset B (40% weight, $1)");
  console.log("-".repeat(70));

  const addBTx = await program.methods
    .addAsset(4000, new BN(1_000_000)) // 40% weight, price $1 (6 dec)
    .accountsStrict({
      authority: payer.publicKey,
      vault,
      assetMint: mintB,
      assetEntry: entryB,
      oracle: oracleB,
      assetVault: assetVaultB,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: SYSVAR_RENT_PUBKEY,
    })
    .remainingAccounts([
      { pubkey: entryA, isSigner: false, isWritable: false },
    ])
    .rpc();

  console.log(`  TX: ${explorerUrl(addBTx)}`);

  // Step 7: Deposit single (Asset A)
  console.log("\n" + "-".repeat(70));
  console.log("Step 7: Deposit single (Asset A)");
  console.log("-".repeat(70));

  const userSharesAta = getAssociatedTokenAddressSync(
    sharesMint, payer.publicKey, false, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID,
  );

  const depositTx = await program.methods
    .depositSingle(new BN(DEPOSIT_AMOUNT * 10 ** 9), new BN(0))
    .accountsStrict({
      user: payer.publicKey,
      vault,
      sharesMint,
      assetEntry: entryA,
      assetMint: mintA,
      assetVault: assetVaultA,
      oracle: oracleA,
      userAssetAccount: userAtaA.address,
      userSharesAccount: userSharesAta,
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .remainingAccounts([
      { pubkey: entryA, isSigner: false, isWritable: false },
      { pubkey: assetVaultA, isSigner: false, isWritable: false },
      { pubkey: oracleA, isSigner: false, isWritable: false },
      { pubkey: entryB, isSigner: false, isWritable: false },
      { pubkey: assetVaultB, isSigner: false, isWritable: false },
      { pubkey: oracleB, isSigner: false, isWritable: false },
    ])
    .preInstructions([createSharesAtaIx(payer.publicKey, payer.publicKey, sharesMint)])
    .rpc();

  console.log(`  TX: ${explorerUrl(depositTx)}`);

  const sharesAccount = await getAccount(connection, userSharesAta, undefined, TOKEN_2022_PROGRAM_ID);
  console.log(`  Shares received: ${sharesAccount.amount.toString()}`);

  // Step 8: Redeem single (Asset A)
  console.log("\n" + "-".repeat(70));
  console.log("Step 8: Redeem single (half shares for Asset A)");
  console.log("-".repeat(70));

  const sharesToRedeem = BigInt(sharesAccount.amount.toString()) / 2n;

  const redeemTx = await program.methods
    .redeemSingle(new BN(sharesToRedeem.toString()), new BN(0))
    .accountsStrict({
      user: payer.publicKey,
      vault,
      sharesMint,
      assetEntry: entryA,
      assetMint: mintA,
      assetVault: assetVaultA,
      oracle: oracleA,
      userAssetAccount: userAtaA.address,
      userSharesAccount: userSharesAta,
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
    })
    .remainingAccounts([
      { pubkey: entryA, isSigner: false, isWritable: false },
      { pubkey: assetVaultA, isSigner: false, isWritable: false },
      { pubkey: oracleA, isSigner: false, isWritable: false },
      { pubkey: entryB, isSigner: false, isWritable: false },
      { pubkey: assetVaultB, isSigner: false, isWritable: false },
      { pubkey: oracleB, isSigner: false, isWritable: false },
    ])
    .rpc();

  console.log(`  TX: ${explorerUrl(redeemTx)}`);

  // Step 9: Pause vault
  console.log("\n" + "-".repeat(70));
  console.log("Step 9: Pause / Unpause");
  console.log("-".repeat(70));

  const pauseTx = await program.methods
    .pause()
    .accountsStrict({ authority: payer.publicKey, vault })
    .rpc();
  console.log(`  Pause TX: ${explorerUrl(pauseTx)}`);

  const unpauseTx = await program.methods
    .unpause()
    .accountsStrict({ authority: payer.publicKey, vault })
    .rpc();
  console.log(`  Unpause TX: ${explorerUrl(unpauseTx)}`);

  // Step 10: Update weights
  console.log("\n" + "-".repeat(70));
  console.log("Step 10: Update weights (50/50)");
  console.log("-".repeat(70));

  const updateTx = await program.methods
    .updateWeights([5000, 5000])
    .accountsStrict({ authority: payer.publicKey, vault })
    .remainingAccounts([
      { pubkey: entryA, isSigner: false, isWritable: true },
      { pubkey: entryB, isSigner: false, isWritable: true },
    ])
    .rpc();
  console.log(`  TX: ${explorerUrl(updateTx)}`);

  console.log("\n" + "=".repeat(70));
  console.log("  ALL TESTS PASSED");
  console.log("=".repeat(70));
  console.log(`\n  Vault: ${accountUrl(vault.toBase58())}`);
  console.log(`  Program: ${accountUrl(programId.toBase58())}\n`);
}

main().catch((err) => {
  console.error("\nFailed:", err);
  process.exit(1);
});
