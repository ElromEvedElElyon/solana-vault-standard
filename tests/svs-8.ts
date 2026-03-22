import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
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
import { Keypair, PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { expect } from "chai";
import { Svs8 } from "../target/types/svs_8";

describe("svs-8 (Multi-Asset Basket Vault)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Svs8 as Program<Svs8>;
  const connection = provider.connection;
  const payer = (provider.wallet as anchor.Wallet).payer;

  // Test state
  let mintA: PublicKey; // SOL-like (9 decimals)
  let mintB: PublicKey; // USDC-like (6 decimals)
  let vault: PublicKey;
  let vaultBump: number;
  let sharesMint: PublicKey;
  let assetEntryA: PublicKey;
  let assetEntryB: PublicKey;
  let oraclePriceA: PublicKey;
  let oraclePriceB: PublicKey;
  let assetVaultA: PublicKey;
  let assetVaultB: PublicKey;
  let userAssetAccountA: PublicKey;
  let userAssetAccountB: PublicKey;
  let userSharesAccount: PublicKey;

  const vaultId = new BN(1);
  const DECIMALS_A = 9;
  const DECIMALS_B = 6;
  const BASE_DECIMALS = 6;

  // Prices in base decimals (6): SOL=$100, USDC=$1
  const PRICE_A = 100_000_000; // $100 with 6 decimals
  const PRICE_B = 1_000_000;   // $1 with 6 decimals

  const getVaultPDA = (vaultId: BN): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("multi_vault"), vaultId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
  };

  const getSharesMintPDA = (vault: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("shares"), vault.toBuffer()],
      program.programId
    );
  };

  const getAssetEntryPDA = (vault: PublicKey, mint: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("asset_entry"), vault.toBuffer(), mint.toBuffer()],
      program.programId
    );
  };

  const getOraclePDA = (vault: PublicKey, mint: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("oracle"), vault.toBuffer(), mint.toBuffer()],
      program.programId
    );
  };

  before(async () => {
    // Create two asset mints
    mintA = await createMint(
      connection, payer, payer.publicKey, null, DECIMALS_A,
      Keypair.generate(), undefined, TOKEN_PROGRAM_ID
    );

    mintB = await createMint(
      connection, payer, payer.publicKey, null, DECIMALS_B,
      Keypair.generate(), undefined, TOKEN_PROGRAM_ID
    );

    [vault, vaultBump] = getVaultPDA(vaultId);
    [sharesMint] = getSharesMintPDA(vault);
    [assetEntryA] = getAssetEntryPDA(vault, mintA);
    [assetEntryB] = getAssetEntryPDA(vault, mintB);
    [oraclePriceA] = getOraclePDA(vault, mintA);
    [oraclePriceB] = getOraclePDA(vault, mintB);

    // Asset vaults are ATAs of vault PDA
    assetVaultA = getAssociatedTokenAddressSync(mintA, vault, true, TOKEN_PROGRAM_ID);
    assetVaultB = getAssociatedTokenAddressSync(mintB, vault, true, TOKEN_PROGRAM_ID);

    // User token accounts
    const userAta_A = await getOrCreateAssociatedTokenAccount(
      connection, payer, mintA, payer.publicKey, false, undefined, undefined, TOKEN_PROGRAM_ID
    );
    userAssetAccountA = userAta_A.address;

    const userAta_B = await getOrCreateAssociatedTokenAccount(
      connection, payer, mintB, payer.publicKey, false, undefined, undefined, TOKEN_PROGRAM_ID
    );
    userAssetAccountB = userAta_B.address;

    // Mint tokens to user: 100 SOL + 10000 USDC
    await mintTo(
      connection, payer, mintA, userAssetAccountA, payer.publicKey,
      100 * 10 ** DECIMALS_A, [], undefined, TOKEN_PROGRAM_ID
    );
    await mintTo(
      connection, payer, mintB, userAssetAccountB, payer.publicKey,
      10_000 * 10 ** DECIMALS_B, [], undefined, TOKEN_PROGRAM_ID
    );

    // User shares account (Token-2022)
    userSharesAccount = getAssociatedTokenAddressSync(
      sharesMint, payer.publicKey, false, TOKEN_2022_PROGRAM_ID
    );
  });

  it("initializes vault", async () => {
    await program.methods
      .initialize(vaultId, BASE_DECIMALS)
      .accounts({
        authority: payer.publicKey,
        vault,
        sharesMint,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.authority.toString()).to.equal(payer.publicKey.toString());
    expect(vaultAccount.sharesMint.toString()).to.equal(sharesMint.toString());
    expect(vaultAccount.numAssets).to.equal(0);
    expect(vaultAccount.paused).to.equal(false);
    expect(vaultAccount.totalShares.toNumber()).to.equal(0);
    expect(vaultAccount.baseDecimals).to.equal(BASE_DECIMALS);
  });

  it("adds asset A (SOL) with 60% weight", async () => {
    // Create user shares ATA (Token-2022)
    await getOrCreateAssociatedTokenAccount(
      connection, payer, sharesMint, payer.publicKey, false,
      undefined, undefined, TOKEN_2022_PROGRAM_ID
    );

    await program.methods
      .addAsset(6000, new BN(PRICE_A)) // 60% weight
      .accounts({
        authority: payer.publicKey,
        vault,
        assetMint: mintA,
        assetEntry: assetEntryA,
        oraclePrice: oraclePriceA,
        assetVault: assetVaultA,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const entry = await program.account.assetEntry.fetch(assetEntryA);
    expect(entry.vault.toString()).to.equal(vault.toString());
    expect(entry.assetMint.toString()).to.equal(mintA.toString());
    expect(entry.targetWeightBps).to.equal(6000);
    expect(entry.assetDecimals).to.equal(DECIMALS_A);
    expect(entry.index).to.equal(0);

    const oracle = await program.account.oraclePrice.fetch(oraclePriceA);
    expect(oracle.price.toNumber()).to.equal(PRICE_A);

    const vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.numAssets).to.equal(1);
  });

  it("adds asset B (USDC) with 40% weight", async () => {
    await program.methods
      .addAsset(4000, new BN(PRICE_B)) // 40% weight
      .accounts({
        authority: payer.publicKey,
        vault,
        assetMint: mintB,
        assetEntry: assetEntryB,
        oraclePrice: oraclePriceB,
        assetVault: assetVaultB,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([
        { pubkey: assetEntryA, isWritable: false, isSigner: false },
      ])
      .rpc();

    const entry = await program.account.assetEntry.fetch(assetEntryB);
    expect(entry.targetWeightBps).to.equal(4000);
    expect(entry.index).to.equal(1);

    const vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.numAssets).to.equal(2);
  });

  it("updates oracle prices", async () => {
    // Update price A (SOL) to $110
    const newPriceA = 110_000_000;
    await program.methods
      .setPrice(new BN(newPriceA))
      .accounts({
        authority: payer.publicKey,
        vault,
        oraclePrice: oraclePriceA,
      })
      .rpc();

    const oracle = await program.account.oraclePrice.fetch(oraclePriceA);
    expect(oracle.price.toNumber()).to.equal(newPriceA);
  });

  it("deposit single asset (1 SOL)", async () => {
    const depositAmount = new BN(1 * 10 ** DECIMALS_A); // 1 SOL

    await program.methods
      .depositSingle(depositAmount, new BN(0)) // min_shares = 0 for first deposit
      .accounts({
        user: payer.publicKey,
        vault,
        sharesMint,
        userSharesAccount,
        assetEntry: assetEntryA,
        assetMint: mintA,
        userAssetAccount: userAssetAccountA,
        assetVault: assetVaultA,
        assetTokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts([
        // Asset A: entry, vault, oracle
        { pubkey: assetEntryA, isWritable: false, isSigner: false },
        { pubkey: assetVaultA, isWritable: false, isSigner: false },
        { pubkey: oraclePriceA, isWritable: false, isSigner: false },
        // Asset B: entry, vault, oracle
        { pubkey: assetEntryB, isWritable: false, isSigner: false },
        { pubkey: assetVaultB, isWritable: false, isSigner: false },
        { pubkey: oraclePriceB, isWritable: false, isSigner: false },
      ])
      .rpc();

    // Verify shares minted
    const sharesAccount = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
    expect(Number(sharesAccount.amount)).to.be.greaterThan(0);

    // Verify asset transferred
    const vaultTokenA = await getAccount(connection, assetVaultA, undefined, TOKEN_PROGRAM_ID);
    expect(Number(vaultTokenA.amount)).to.equal(1 * 10 ** DECIMALS_A);

    const vaultData = await program.account.multiAssetVault.fetch(vault);
    expect(vaultData.totalShares.toNumber()).to.be.greaterThan(0);
  });

  it("deposit single asset B (100 USDC)", async () => {
    const depositAmount = new BN(100 * 10 ** DECIMALS_B);

    await program.methods
      .depositSingle(depositAmount, new BN(0))
      .accounts({
        user: payer.publicKey,
        vault,
        sharesMint,
        userSharesAccount,
        assetEntry: assetEntryB,
        assetMint: mintB,
        userAssetAccount: userAssetAccountB,
        assetVault: assetVaultB,
        assetTokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts([
        { pubkey: assetEntryA, isWritable: false, isSigner: false },
        { pubkey: assetVaultA, isWritable: false, isSigner: false },
        { pubkey: oraclePriceA, isWritable: false, isSigner: false },
        { pubkey: assetEntryB, isWritable: false, isSigner: false },
        { pubkey: assetVaultB, isWritable: false, isSigner: false },
        { pubkey: oraclePriceB, isWritable: false, isSigner: false },
      ])
      .rpc();

    const vaultTokenB = await getAccount(connection, assetVaultB, undefined, TOKEN_PROGRAM_ID);
    expect(Number(vaultTokenB.amount)).to.equal(100 * 10 ** DECIMALS_B);
  });

  it("redeem single (partial shares for SOL)", async () => {
    const sharesAccount = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
    const totalShares = Number(sharesAccount.amount);
    const redeemShares = new BN(Math.floor(totalShares / 4)); // 25%

    const balanceBefore = await getAccount(connection, userAssetAccountA, undefined, TOKEN_PROGRAM_ID);

    await program.methods
      .redeemSingle(redeemShares, new BN(0)) // min_assets_out = 0
      .accounts({
        user: payer.publicKey,
        vault,
        sharesMint,
        userSharesAccount,
        assetEntry: assetEntryA,
        assetMint: mintA,
        userAssetAccount: userAssetAccountA,
        assetVault: assetVaultA,
        assetTokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts([
        { pubkey: assetEntryA, isWritable: false, isSigner: false },
        { pubkey: assetVaultA, isWritable: false, isSigner: false },
        { pubkey: oraclePriceA, isWritable: false, isSigner: false },
        { pubkey: assetEntryB, isWritable: false, isSigner: false },
        { pubkey: assetVaultB, isWritable: false, isSigner: false },
        { pubkey: oraclePriceB, isWritable: false, isSigner: false },
      ])
      .rpc();

    const balanceAfter = await getAccount(connection, userAssetAccountA, undefined, TOKEN_PROGRAM_ID);
    expect(Number(balanceAfter.amount)).to.be.greaterThan(Number(balanceBefore.amount));
  });

  it("pause and unpause", async () => {
    await program.methods
      .pause()
      .accounts({ authority: payer.publicKey, vault })
      .rpc();

    let vaultData = await program.account.multiAssetVault.fetch(vault);
    expect(vaultData.paused).to.equal(true);

    await program.methods
      .unpause()
      .accounts({ authority: payer.publicKey, vault })
      .rpc();

    vaultData = await program.account.multiAssetVault.fetch(vault);
    expect(vaultData.paused).to.equal(false);
  });

  it("update weights", async () => {
    await program.methods
      .updateWeights([5000, 5000]) // 50/50 split
      .accounts({
        authority: payer.publicKey,
        vault,
      })
      .remainingAccounts([
        { pubkey: assetEntryA, isWritable: true, isSigner: false },
        { pubkey: assetEntryB, isWritable: true, isSigner: false },
      ])
      .rpc();

    const entryA = await program.account.assetEntry.fetch(assetEntryA);
    const entryB = await program.account.assetEntry.fetch(assetEntryB);
    expect(entryA.targetWeightBps).to.equal(5000);
    expect(entryB.targetWeightBps).to.equal(5000);
  });

  it("transfer authority", async () => {
    const newAuthority = Keypair.generate();

    await program.methods
      .transferAuthority(newAuthority.publicKey)
      .accounts({ authority: payer.publicKey, vault })
      .rpc();

    let vaultData = await program.account.multiAssetVault.fetch(vault);
    expect(vaultData.authority.toString()).to.equal(newAuthority.publicKey.toString());

    // Transfer back
    await program.methods
      .transferAuthority(payer.publicKey)
      .accounts({ authority: newAuthority.publicKey, vault })
      .signers([newAuthority])
      .rpc();

    vaultData = await program.account.multiAssetVault.fetch(vault);
    expect(vaultData.authority.toString()).to.equal(payer.publicKey.toString());
  });

  it("rejects deposit when paused", async () => {
    await program.methods.pause().accounts({ authority: payer.publicKey, vault }).rpc();

    try {
      await program.methods
        .depositSingle(new BN(1000), new BN(0))
        .accounts({
          user: payer.publicKey,
          vault,
          sharesMint,
          userSharesAccount,
          assetEntry: assetEntryA,
          assetMint: mintA,
          userAssetAccount: userAssetAccountA,
          assetVault: assetVaultA,
          assetTokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
        })
        .remainingAccounts([
          { pubkey: assetEntryA, isWritable: false, isSigner: false },
          { pubkey: assetVaultA, isWritable: false, isSigner: false },
          { pubkey: oraclePriceA, isWritable: false, isSigner: false },
          { pubkey: assetEntryB, isWritable: false, isSigner: false },
          { pubkey: assetVaultB, isWritable: false, isSigner: false },
          { pubkey: oraclePriceB, isWritable: false, isSigner: false },
        ])
        .rpc();
      expect.fail("should have thrown");
    } catch (e: any) {
      expect(e.error?.errorCode?.code || e.message).to.contain("VaultPaused");
    }

    await program.methods.unpause().accounts({ authority: payer.publicKey, vault }).rpc();
  });

  it("rejects weights that don't sum to 10000", async () => {
    try {
      await program.methods
        .updateWeights([3000, 3000]) // Only 6000, not 10000
        .accounts({ authority: payer.publicKey, vault })
        .remainingAccounts([
          { pubkey: assetEntryA, isWritable: true, isSigner: false },
          { pubkey: assetEntryB, isWritable: true, isSigner: false },
        ])
        .rpc();
      expect.fail("should have thrown");
    } catch (e: any) {
      expect(e.error?.errorCode?.code || e.message).to.contain("InvalidWeight");
    }
  });
});
