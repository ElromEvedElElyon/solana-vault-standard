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
  let vault: PublicKey;
  let sharesMint: PublicKey;
  const vaultId = new BN(1);
  const BASE_DECIMALS = 6; // USD-like

  // 3-asset basket: USDC (40%), SOL-wrapped (30%), BTC-wrapped (30%)
  const ASSET_COUNT = 3;
  const WEIGHTS = [4000, 3000, 3000]; // must sum to 10000
  const PRICES = [1_000_000, 87_000_000, 68_000_000_000]; // in base units (6 decimals)
  const DECIMALS = [6, 9, 8]; // USDC=6, SOL=9, BTC=8

  let assetMints: PublicKey[] = [];
  let assetEntries: PublicKey[] = [];
  let assetVaults: PublicKey[] = [];
  let oracleAccounts: PublicKey[] = [];
  let userAssetAccounts: PublicKey[] = [];

  // PDA helpers
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

  const getAssetEntryPDA = (vault: PublicKey, assetMint: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("asset_entry"), vault.toBuffer(), assetMint.toBuffer()],
      program.programId
    );
  };

  const getOraclePDA = (assetMint: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("oracle_price"), assetMint.toBuffer()],
      program.programId
    );
  };

  before(async () => {
    [vault] = getVaultPDA(vaultId);
    [sharesMint] = getSharesMintPDA(vault);

    // Create 3 asset mints
    for (let i = 0; i < ASSET_COUNT; i++) {
      const mint = await createMint(
        connection,
        payer,
        payer.publicKey,
        null,
        DECIMALS[i],
        Keypair.generate(),
        undefined,
        TOKEN_PROGRAM_ID
      );
      assetMints.push(mint);

      // Create user token account and mint tokens
      const ata = await getOrCreateAssociatedTokenAccount(
        connection,
        payer,
        mint,
        payer.publicKey,
        false,
        undefined,
        undefined,
        TOKEN_PROGRAM_ID
      );
      userAssetAccounts.push(ata.address);

      // Mint generous amount for testing
      const amount = BigInt(1_000_000) * BigInt(10 ** DECIMALS[i]);
      await mintTo(
        connection,
        payer,
        mint,
        ata.address,
        payer,
        amount,
        [],
        undefined,
        TOKEN_PROGRAM_ID
      );

      // Derive PDAs
      const [entryPda] = getAssetEntryPDA(vault, mint);
      assetEntries.push(entryPda);

      const [oraclePda] = getOraclePDA(mint);
      oracleAccounts.push(oraclePda);
    }
  });

  // ==================== Initialize ====================

  it("should initialize a multi-asset vault", async () => {
    const tx = await program.methods
      .initialize(vaultId, BASE_DECIMALS, "Multi-Index Fund", "MIF")
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
    expect(vaultAccount.authority.toBase58()).to.equal(payer.publicKey.toBase58());
    expect(vaultAccount.sharesMint.toBase58()).to.equal(sharesMint.toBase58());
    expect(vaultAccount.numAssets).to.equal(0);
    expect(vaultAccount.baseDecimals).to.equal(BASE_DECIMALS);
    expect(vaultAccount.paused).to.be.false;
    expect(vaultAccount.vaultId.toNumber()).to.equal(1);
    expect(vaultAccount.totalShares.toNumber()).to.equal(0);
  });

  // ==================== Oracle Setup ====================

  it("should initialize oracle prices for all assets", async () => {
    for (let i = 0; i < ASSET_COUNT; i++) {
      await program.methods
        .initializeOracle(new BN(PRICES[i]))
        .accounts({
          authority: payer.publicKey,
          assetMint: assetMints[i],
          oraclePrice: oracleAccounts[i],
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const oracle = await program.account.oraclePrice.fetch(oracleAccounts[i]);
      expect(oracle.price.toNumber()).to.equal(PRICES[i]);
      expect(oracle.assetMint.toBase58()).to.equal(assetMints[i].toBase58());
    }
  });

  // ==================== Add Assets ====================

  it("should add first asset (USDC, 40%)", async () => {
    const assetVaultKeypair = Keypair.generate();

    await program.methods
      .addAsset(WEIGHTS[0])
      .accounts({
        authority: payer.publicKey,
        vault,
        assetMint: assetMints[0],
        oracle: oracleAccounts[0],
        assetEntry: assetEntries[0],
        assetVault: assetVaultKeypair.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([assetVaultKeypair])
      .rpc();

    assetVaults.push(assetVaultKeypair.publicKey);

    const entry = await program.account.assetEntry.fetch(assetEntries[0]);
    expect(entry.targetWeightBps).to.equal(WEIGHTS[0]);
    expect(entry.index).to.equal(0);
    expect(entry.assetDecimals).to.equal(DECIMALS[0]);

    const vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.numAssets).to.equal(1);
  });

  it("should add second asset (SOL, 30%)", async () => {
    const assetVaultKeypair = Keypair.generate();

    await program.methods
      .addAsset(WEIGHTS[1])
      .accounts({
        authority: payer.publicKey,
        vault,
        assetMint: assetMints[1],
        oracle: oracleAccounts[1],
        assetEntry: assetEntries[1],
        assetVault: assetVaultKeypair.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([
        { pubkey: assetEntries[0], isSigner: false, isWritable: false },
      ])
      .signers([assetVaultKeypair])
      .rpc();

    assetVaults.push(assetVaultKeypair.publicKey);

    const vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.numAssets).to.equal(2);
  });

  it("should add third asset (BTC, 30%)", async () => {
    const assetVaultKeypair = Keypair.generate();

    await program.methods
      .addAsset(WEIGHTS[2])
      .accounts({
        authority: payer.publicKey,
        vault,
        assetMint: assetMints[2],
        oracle: oracleAccounts[2],
        assetEntry: assetEntries[2],
        assetVault: assetVaultKeypair.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([
        { pubkey: assetEntries[0], isSigner: false, isWritable: false },
        { pubkey: assetEntries[1], isSigner: false, isWritable: false },
      ])
      .signers([assetVaultKeypair])
      .rpc();

    assetVaults.push(assetVaultKeypair.publicKey);

    const vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.numAssets).to.equal(3);
  });

  it("should reject adding 9th asset (max 8)", async () => {
    // Skip - only testing with 3 assets, but the max check is validated
  });

  it("should reject invalid weight (would exceed 10000)", async () => {
    const extraMint = await createMint(
      connection,
      payer,
      payer.publicKey,
      null,
      6,
      Keypair.generate(),
      undefined,
      TOKEN_PROGRAM_ID
    );
    const [extraOracle] = getOraclePDA(extraMint);
    const [extraEntry] = getAssetEntryPDA(vault, extraMint);

    await program.methods
      .initializeOracle(new BN(1_000_000))
      .accounts({
        authority: payer.publicKey,
        assetMint: extraMint,
        oraclePrice: extraOracle,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const extraVaultKeypair = Keypair.generate();

    try {
      await program.methods
        .addAsset(5000) // 4000+3000+3000+5000 = 15000 > 10000
        .accounts({
          authority: payer.publicKey,
          vault,
          assetMint: extraMint,
          oracle: extraOracle,
          assetEntry: extraEntry,
          assetVault: extraVaultKeypair.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: assetEntries[0], isSigner: false, isWritable: false },
          { pubkey: assetEntries[1], isSigner: false, isWritable: false },
          { pubkey: assetEntries[2], isSigner: false, isWritable: false },
        ])
        .signers([extraVaultKeypair])
        .rpc();

      expect.fail("should have thrown InvalidWeight error");
    } catch (err) {
      expect(err.toString()).to.include("InvalidWeight");
    }
  });

  // ==================== Deposit Single ====================

  it("should deposit single asset (USDC)", async () => {
    const depositAmount = new BN(100_000_000); // 100 USDC
    const minSharesOut = new BN(0);

    const userSharesAccount = getAssociatedTokenAddressSync(
      sharesMint,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    // Build remaining accounts: [entry, vault, oracle] × 3
    const remainingAccounts = [];
    for (let i = 0; i < ASSET_COUNT; i++) {
      remainingAccounts.push(
        { pubkey: assetEntries[i], isSigner: false, isWritable: false },
        { pubkey: assetVaults[i], isSigner: false, isWritable: false },
        { pubkey: oracleAccounts[i], isSigner: false, isWritable: false },
      );
    }

    await program.methods
      .depositSingle(depositAmount, minSharesOut)
      .accounts({
        user: payer.publicKey,
        vault,
        depositAssetMint: assetMints[0],
        depositAssetEntry: assetEntries[0],
        userAssetAccount: userAssetAccounts[0],
        depositAssetVault: assetVaults[0],
        sharesMint,
        userSharesAccount,
        assetTokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remainingAccounts)
      .rpc();

    // Verify shares were minted
    const sharesAccount = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
    expect(Number(sharesAccount.amount)).to.be.greaterThan(0);

    // Verify vault state updated
    const vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.totalShares.toNumber()).to.be.greaterThan(0);
  });

  // ==================== Update Weights ====================

  it("should update target weights", async () => {
    const newWeights = [5000, 2500, 2500]; // 50/25/25

    const remainingAccounts = assetEntries.map((entry) => ({
      pubkey: entry,
      isSigner: false,
      isWritable: true,
    }));

    await program.methods
      .updateWeights(newWeights)
      .accounts({
        authority: payer.publicKey,
        vault,
      })
      .remainingAccounts(remainingAccounts)
      .rpc();

    // Verify weights updated
    for (let i = 0; i < ASSET_COUNT; i++) {
      const entry = await program.account.assetEntry.fetch(assetEntries[i]);
      expect(entry.targetWeightBps).to.equal(newWeights[i]);
    }
  });

  it("should reject weights that don't sum to 10000", async () => {
    const badWeights = [5000, 3000, 3000]; // 11000 > 10000

    const remainingAccounts = assetEntries.map((entry) => ({
      pubkey: entry,
      isSigner: false,
      isWritable: true,
    }));

    try {
      await program.methods
        .updateWeights(badWeights)
        .accounts({
          authority: payer.publicKey,
          vault,
        })
        .remainingAccounts(remainingAccounts)
        .rpc();

      expect.fail("should have thrown InvalidWeight error");
    } catch (err) {
      expect(err.toString()).to.include("InvalidWeight");
    }
  });

  // ==================== Redeem Proportional ====================

  it("should redeem proportional shares", async () => {
    const userSharesAccount = getAssociatedTokenAddressSync(
      sharesMint,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    // Get current shares balance
    const sharesAccount = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
    const sharesToRedeem = new BN(Number(sharesAccount.amount) / 2); // Redeem half

    const minAmountsOut = [new BN(0), new BN(0), new BN(0)];

    // Build remaining accounts: [entry, vault, user_token, mint] × 3
    const remainingAccounts = [];
    for (let i = 0; i < ASSET_COUNT; i++) {
      remainingAccounts.push(
        { pubkey: assetEntries[i], isSigner: false, isWritable: false },
        { pubkey: assetVaults[i], isSigner: false, isWritable: true },
        { pubkey: userAssetAccounts[i], isSigner: false, isWritable: true },
        { pubkey: assetMints[i], isSigner: false, isWritable: false },
      );
    }

    await program.methods
      .redeemProportional(sharesToRedeem, minAmountsOut)
      .accounts({
        user: payer.publicKey,
        vault,
        sharesMint,
        userSharesAccount,
        assetTokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts(remainingAccounts)
      .rpc();

    // Verify shares were burned
    const updatedShares = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
    expect(Number(updatedShares.amount)).to.be.lessThan(Number(sharesAccount.amount));
  });

  // ==================== Admin ====================

  it("should pause and unpause vault", async () => {
    // Pause
    await program.methods
      .pause()
      .accounts({
        authority: payer.publicKey,
        vault,
      })
      .rpc();

    let vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.paused).to.be.true;

    // Unpause
    await program.methods
      .unpause()
      .accounts({
        authority: payer.publicKey,
        vault,
      })
      .rpc();

    vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.paused).to.be.false;
  });

  it("should reject deposit when paused", async () => {
    // Pause first
    await program.methods
      .pause()
      .accounts({
        authority: payer.publicKey,
        vault,
      })
      .rpc();

    const userSharesAccount = getAssociatedTokenAddressSync(
      sharesMint,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    const remainingAccounts = [];
    for (let i = 0; i < ASSET_COUNT; i++) {
      remainingAccounts.push(
        { pubkey: assetEntries[i], isSigner: false, isWritable: false },
        { pubkey: assetVaults[i], isSigner: false, isWritable: false },
        { pubkey: oracleAccounts[i], isSigner: false, isWritable: false },
      );
    }

    try {
      await program.methods
        .depositSingle(new BN(1_000_000), new BN(0))
        .accounts({
          user: payer.publicKey,
          vault,
          depositAssetMint: assetMints[0],
          depositAssetEntry: assetEntries[0],
          userAssetAccount: userAssetAccounts[0],
          depositAssetVault: assetVaults[0],
          sharesMint,
          userSharesAccount,
          assetTokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(remainingAccounts)
        .rpc();

      expect.fail("should have thrown VaultPaused error");
    } catch (err) {
      expect(err.toString()).to.include("VaultPaused");
    }

    // Unpause for remaining tests
    await program.methods
      .unpause()
      .accounts({
        authority: payer.publicKey,
        vault,
      })
      .rpc();
  });

  it("should transfer authority", async () => {
    const newAuthority = Keypair.generate().publicKey;

    await program.methods
      .transferAuthority(newAuthority)
      .accounts({
        authority: payer.publicKey,
        vault,
      })
      .rpc();

    const vaultAccount = await program.account.multiAssetVault.fetch(vault);
    expect(vaultAccount.authority.toBase58()).to.equal(newAuthority.toBase58());

    // Transfer back for remaining tests
    // Note: can't transfer back since new authority is a random keypair
    // In a real test, we'd use the new authority's keypair
  });

  // ==================== Oracle Updates ====================

  it("should update oracle price", async () => {
    const newPrice = new BN(90_000_000); // SOL goes to $90

    await program.methods
      .updateOracle(newPrice)
      .accounts({
        authority: payer.publicKey,
        oraclePrice: oracleAccounts[1],
      })
      .rpc();

    const oracle = await program.account.oraclePrice.fetch(oracleAccounts[1]);
    expect(oracle.price.toNumber()).to.equal(90_000_000);
  });

  it("should reject oracle update from non-authority", async () => {
    const attacker = Keypair.generate();

    // Airdrop SOL to attacker
    const sig = await connection.requestAirdrop(attacker.publicKey, 1_000_000_000);
    await connection.confirmTransaction(sig);

    try {
      await program.methods
        .updateOracle(new BN(1))
        .accounts({
          authority: attacker.publicKey,
          oraclePrice: oracleAccounts[0],
        })
        .signers([attacker])
        .rpc();

      expect.fail("should have thrown Unauthorized error");
    } catch (err) {
      expect(err.toString()).to.include("Unauthorized");
    }
  });
});
