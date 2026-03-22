/**
 * SVS-8 Basket Vault SDK
 *
 * TypeScript client for interacting with Multi-Asset Basket Vaults.
 * Supports initialization, asset management, deposits, redemptions,
 * and oracle price queries.
 *
 * @example
 * ```ts
 * import { BasketVault } from "./basket-vault";
 *
 * const vault = await BasketVault.load(program, vaultId);
 * console.log(`Portfolio value: $${vault.totalPortfolioValue()}`);
 *
 * // Deposit single asset
 * const tx = await vault.depositSingle(user, {
 *   assetMint: usdcMint,
 *   amount: new BN(100_000_000),
 *   minSharesOut: new BN(0),
 * });
 * ```
 */

import { BN, Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

// PDA seed constants (must match program)
const MULTI_VAULT_SEED = Buffer.from("multi_vault");
const ASSET_ENTRY_SEED = Buffer.from("asset_entry");
const SHARES_MINT_SEED = Buffer.from("shares");
const ORACLE_PRICE_SEED = Buffer.from("oracle_price");

const BPS_DENOMINATOR = 10_000;

/**
 * Asset configuration within a basket vault
 */
export interface BasketAsset {
  mint: PublicKey;
  entryPda: PublicKey;
  vaultAccount: PublicKey;
  oracle: PublicKey;
  targetWeightBps: number;
  decimals: number;
  index: number;
}

/**
 * Basket vault state
 */
export interface BasketVaultState {
  authority: PublicKey;
  sharesMint: PublicKey;
  totalShares: BN;
  decimalsOffset: number;
  paused: boolean;
  vaultId: BN;
  numAssets: number;
  baseDecimals: number;
}

/**
 * SVS-8 Multi-Asset Basket Vault SDK
 */
export class BasketVault {
  public readonly programId: PublicKey;
  public readonly vaultPda: PublicKey;
  public readonly vaultBump: number;
  public readonly sharesMintPda: PublicKey;
  public readonly sharesMintBump: number;
  public state: BasketVaultState | null = null;
  public assets: BasketAsset[] = [];

  private constructor(
    public readonly program: Program<any>,
    public readonly vaultId: BN,
  ) {
    this.programId = program.programId;

    const [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [MULTI_VAULT_SEED, vaultId.toArrayLike(Buffer, "le", 8)],
      this.programId,
    );
    this.vaultPda = vaultPda;
    this.vaultBump = vaultBump;

    const [sharesMintPda, sharesMintBump] = PublicKey.findProgramAddressSync(
      [SHARES_MINT_SEED, vaultPda.toBuffer()],
      this.programId,
    );
    this.sharesMintPda = sharesMintPda;
    this.sharesMintBump = sharesMintBump;
  }

  /**
   * Load an existing basket vault from on-chain state
   */
  static async load(program: Program<any>, vaultId: BN): Promise<BasketVault> {
    const vault = new BasketVault(program, vaultId);
    await vault.refresh();
    return vault;
  }

  /**
   * Create a new basket vault
   */
  static async create(
    program: Program<any>,
    params: {
      vaultId: BN;
      baseDecimals: number;
      name: string;
      symbol: string;
      authority: PublicKey;
    },
  ): Promise<BasketVault> {
    const vault = new BasketVault(program, params.vaultId);

    await program.methods
      .initialize(params.vaultId, params.baseDecimals, params.name, params.symbol)
      .accounts({
        authority: params.authority,
        vault: vault.vaultPda,
        sharesMint: vault.sharesMintPda,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    await vault.refresh();
    return vault;
  }

  /**
   * Refresh vault state from on-chain
   */
  async refresh(): Promise<void> {
    const account = await this.program.account.multiAssetVault.fetch(this.vaultPda);
    this.state = {
      authority: account.authority,
      sharesMint: account.sharesMint,
      totalShares: account.totalShares,
      decimalsOffset: account.decimalsOffset,
      paused: account.paused,
      vaultId: account.vaultId,
      numAssets: account.numAssets,
      baseDecimals: account.baseDecimals,
    };

    // Load all asset entries
    this.assets = [];
    // Note: In production, use getProgramAccounts with filters
  }

  // ==================== PDA Helpers ====================

  static getVaultPDA(programId: PublicKey, vaultId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [MULTI_VAULT_SEED, vaultId.toArrayLike(Buffer, "le", 8)],
      programId,
    );
  }

  static getSharesMintPDA(programId: PublicKey, vault: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [SHARES_MINT_SEED, vault.toBuffer()],
      programId,
    );
  }

  static getAssetEntryPDA(
    programId: PublicKey,
    vault: PublicKey,
    assetMint: PublicKey,
  ): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [ASSET_ENTRY_SEED, vault.toBuffer(), assetMint.toBuffer()],
      programId,
    );
  }

  static getOraclePDA(programId: PublicKey, assetMint: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [ORACLE_PRICE_SEED, assetMint.toBuffer()],
      programId,
    );
  }

  // ==================== Asset Management ====================

  /**
   * Add an asset to the basket
   */
  async addAsset(params: {
    authority: PublicKey;
    assetMint: PublicKey;
    oracle: PublicKey;
    assetVaultKeypair: any; // Keypair
    targetWeightBps: number;
    existingEntries?: PublicKey[];
  }): Promise<string> {
    const [assetEntry] = BasketVault.getAssetEntryPDA(
      this.programId,
      this.vaultPda,
      params.assetMint,
    );

    const remainingAccounts = (params.existingEntries ?? []).map((entry) => ({
      pubkey: entry,
      isSigner: false,
      isWritable: false,
    }));

    return await this.program.methods
      .addAsset(params.targetWeightBps)
      .accounts({
        authority: params.authority,
        vault: this.vaultPda,
        assetMint: params.assetMint,
        oracle: params.oracle,
        assetEntry,
        assetVault: params.assetVaultKeypair.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remainingAccounts)
      .signers([params.assetVaultKeypair])
      .rpc();
  }

  /**
   * Update target weights for all assets
   */
  async updateWeights(params: {
    authority: PublicKey;
    newWeights: number[];
    assetEntries: PublicKey[];
  }): Promise<string> {
    const remainingAccounts = params.assetEntries.map((entry) => ({
      pubkey: entry,
      isSigner: false,
      isWritable: true,
    }));

    return await this.program.methods
      .updateWeights(params.newWeights)
      .accounts({
        authority: params.authority,
        vault: this.vaultPda,
      })
      .remainingAccounts(remainingAccounts)
      .rpc();
  }

  // ==================== Deposit / Redeem ====================

  /**
   * Deposit a single asset into the basket vault
   */
  async depositSingle(
    user: PublicKey,
    params: {
      assetMint: PublicKey;
      assetEntry: PublicKey;
      userAssetAccount: PublicKey;
      assetVault: PublicKey;
      amount: BN;
      minSharesOut: BN;
      allEntries: PublicKey[];
      allVaults: PublicKey[];
      allOracles: PublicKey[];
    },
  ): Promise<string> {
    const userSharesAccount = getAssociatedTokenAddressSync(
      this.sharesMintPda,
      user,
      false,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    // Build remaining accounts: [entry, vault, oracle] × num_assets
    const remainingAccounts = [];
    for (let i = 0; i < params.allEntries.length; i++) {
      remainingAccounts.push(
        { pubkey: params.allEntries[i], isSigner: false, isWritable: false },
        { pubkey: params.allVaults[i], isSigner: false, isWritable: false },
        { pubkey: params.allOracles[i], isSigner: false, isWritable: false },
      );
    }

    return await this.program.methods
      .depositSingle(params.amount, params.minSharesOut)
      .accounts({
        user,
        vault: this.vaultPda,
        depositAssetMint: params.assetMint,
        depositAssetEntry: params.assetEntry,
        userAssetAccount: params.userAssetAccount,
        depositAssetVault: params.assetVault,
        sharesMint: this.sharesMintPda,
        userSharesAccount,
        assetTokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remainingAccounts)
      .rpc();
  }

  /**
   * Redeem shares proportionally across all basket assets
   */
  async redeemProportional(
    user: PublicKey,
    params: {
      shares: BN;
      minAmountsOut: BN[];
      allEntries: PublicKey[];
      allVaults: PublicKey[];
      allUserTokens: PublicKey[];
      allMints: PublicKey[];
    },
  ): Promise<string> {
    const userSharesAccount = getAssociatedTokenAddressSync(
      this.sharesMintPda,
      user,
      false,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    // Build remaining accounts: [entry, vault, user_token, mint] × num_assets
    const remainingAccounts = [];
    for (let i = 0; i < params.allEntries.length; i++) {
      remainingAccounts.push(
        { pubkey: params.allEntries[i], isSigner: false, isWritable: false },
        { pubkey: params.allVaults[i], isSigner: false, isWritable: true },
        { pubkey: params.allUserTokens[i], isSigner: false, isWritable: true },
        { pubkey: params.allMints[i], isSigner: false, isWritable: false },
      );
    }

    return await this.program.methods
      .redeemProportional(params.shares, params.minAmountsOut)
      .accounts({
        user,
        vault: this.vaultPda,
        sharesMint: this.sharesMintPda,
        userSharesAccount,
        assetTokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts(remainingAccounts)
      .rpc();
  }

  // ==================== Admin ====================

  async pause(authority: PublicKey): Promise<string> {
    return await this.program.methods
      .pause()
      .accounts({ authority, vault: this.vaultPda })
      .rpc();
  }

  async unpause(authority: PublicKey): Promise<string> {
    return await this.program.methods
      .unpause()
      .accounts({ authority, vault: this.vaultPda })
      .rpc();
  }

  async transferAuthority(authority: PublicKey, newAuthority: PublicKey): Promise<string> {
    return await this.program.methods
      .transferAuthority(newAuthority)
      .accounts({ authority, vault: this.vaultPda })
      .rpc();
  }

  // ==================== Validation ====================

  /**
   * Validate that weights sum to 10000 bps
   */
  static validateWeights(weights: number[]): boolean {
    const total = weights.reduce((sum, w) => sum + w, 0);
    return total === BPS_DENOMINATOR;
  }
}
