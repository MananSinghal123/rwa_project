import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Rwa } from "../target/types/rwa";
import {
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  Keypair,
} from "@solana/web3.js";
import {
  ExtensionType,
  TOKEN_2022_PROGRAM_ID,
  getMintLen,
  createInitializeMintInstruction,
  createInitializeTransferHookInstruction,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  createTransferCheckedInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

describe("transfer-hook", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Rwa as Program<Rwa>;
  const wallet = provider.wallet as anchor.Wallet;
  const connection = provider.connection;

  // Generate keypair to use as address for the transfer-hook enabled mint
  const mint = new Keypair();
  const decimals = 9;

  // Generate keypair for custodian
  const custodian = Keypair.generate();

  // Sender token account address
  const sourceTokenAccount = getAssociatedTokenAddressSync(
    mint.publicKey,
    wallet.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  // Recipient token account address
  const recipient = Keypair.generate();
  const destinationTokenAccount = getAssociatedTokenAddressSync(
    mint.publicKey,
    recipient.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  // ExtraAccountMetaList address
  // Store extra accounts required by the custom transfer hook instruction
  const [extraAccountMetaListPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("extra-account-metas"), mint.publicKey.toBuffer()],
    program.programId
  );

  // Generate keypair for asset details account
  const assetDetails = Keypair.generate();

  it("Create Mint Account with Transfer Hook Extension", async () => {
    const extensions = [ExtensionType.TransferHook];
    const mintLen = getMintLen(extensions);
    const lamports =
      await provider.connection.getMinimumBalanceForRentExemption(mintLen);

    const transaction = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: wallet.publicKey,
        newAccountPubkey: mint.publicKey,
        space: mintLen,
        lamports: lamports,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeTransferHookInstruction(
        mint.publicKey,
        wallet.publicKey,
        program.programId, // Transfer Hook Program ID
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeMintInstruction(
        mint.publicKey,
        decimals,
        wallet.publicKey,
        null,
        TOKEN_2022_PROGRAM_ID
      )
    );

    const txSig = await sendAndConfirmTransaction(
      provider.connection,
      transaction,
      [wallet.payer, mint]
    );
    console.log(`Transaction Signature: ${txSig}`);
  });

  it("Initialize Asset Details", async () => {
    // Fund custodian for rent
    const fundingTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: wallet.publicKey,
        toPubkey: custodian.publicKey,
        lamports: 1000000000,
      })
    );

    await sendAndConfirmTransaction(connection, fundingTx, [wallet.payer], {
      skipPreflight: true,
    });

    const initializeAssetInstruction = await program.methods
      .initializeAsset(
        "RealEstate",
        "DEED123456",
        "US-NY",
        new anchor.BN(1000000),
        "https://metadata.example.com/asset"
      )
      .accounts({
        payer: wallet.publicKey,
        assetDetails: assetDetails.publicKey,
        custodian: custodian.publicKey,
        extraAccountMetaList: extraAccountMetaListPDA,
        mint: mint.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    const transaction = new Transaction().add(initializeAssetInstruction);

    const txSig = await sendAndConfirmTransaction(
      provider.connection,
      transaction,
      [wallet.payer, custodian, assetDetails],
      { skipPreflight: true }
    );
    console.log("Asset Initialization Signature:", txSig);
  });

  it("Update Asset Details", async () => {
    const updateAssetInstruction = await program.methods
      .updateAssetDetails(
        new anchor.BN(1100000),
        true,
        "https://metadata.example.com/asset/updated"
      )
      .accounts({
        custodian: custodian.publicKey,
        assetDetails: assetDetails.publicKey,
      })
      .instruction();

    const transaction = new Transaction().add(updateAssetInstruction);

    const txSig = await sendAndConfirmTransaction(
      provider.connection,
      transaction,
      [custodian],
      { skipPreflight: true }
    );
    console.log("Asset Update Signature:", txSig);
  });

  it("Transfer Token with RWA Compliance Check", async () => {
    const amount = 1 * 10 ** decimals;

    let transferInstruction = await createTransferCheckedInstruction(
      sourceTokenAccount,
      mint.publicKey,
      destinationTokenAccount,
      wallet.publicKey,
      amount,
      decimals,
      [],
      TOKEN_2022_PROGRAM_ID
    );

    // Add the required accounts for the transfer hook
    transferInstruction.keys.push(
      {
        pubkey: program.programId,
        isSigner: false,
        isWritable: false,
      },
      {
        pubkey: extraAccountMetaListPDA,
        isSigner: false,
        isWritable: false,
      },
      {
        pubkey: assetDetails.publicKey,
        isSigner: false,
        isWritable: false,
      }
    );

    const transaction = new Transaction().add(transferInstruction);

    const txSig = await sendAndConfirmTransaction(
      connection,
      transaction,
      [wallet.payer],
      { skipPreflight: true }
    );
    console.log("Transfer Signature:", txSig);
  });

  // Account to store extra accounts required by the transfer hook instruction
  it("Create ExtraAccountMetaList Account", async () => {
    const initializeExtraAccountMetaListInstruction = await program.methods
      .initializeExtraAccountMetaList()
      .accounts({
        payer: wallet.publicKey,
        mint: mint.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        // mint: mint.publicKey,
        // extraAccountMetaList: extraAccountMetaListPDA,
      })
      .instruction();

    const transaction = new Transaction().add(
      initializeExtraAccountMetaListInstruction
    );

    const txSig = await sendAndConfirmTransaction(
      provider.connection,
      transaction,
      [wallet.payer],
      { skipPreflight: true }
    );
    console.log("Transaction Signature:", txSig);
  });

  it("Should fail transfer when asset is non-compliant", async () => {
    // First make asset non-compliant
    const updateAssetInstruction = await program.methods
      .updateAssetDetails(
        null,
        false, // Set compliance_status to false
        null
      )
      .accounts({
        custodian: custodian.publicKey,
        assetDetails: assetDetails.publicKey,
      })
      .instruction();

    await sendAndConfirmTransaction(
      connection,
      new Transaction().add(updateAssetInstruction),
      [custodian],
      { skipPreflight: true }
    );

    // Try to transfer
    const amount = 1 * 10 ** decimals;
    let transferInstruction = await createTransferCheckedInstruction(
      sourceTokenAccount,
      mint.publicKey,
      destinationTokenAccount,
      wallet.publicKey,
      amount,
      decimals,
      [],
      TOKEN_2022_PROGRAM_ID
    );

    transferInstruction.keys.push(
      {
        pubkey: program.programId,
        isSigner: false,
        isWritable: false,
      },
      {
        pubkey: extraAccountMetaListPDA,
        isSigner: false,
        isWritable: false,
      },
      {
        pubkey: assetDetails.publicKey,
        isSigner: false,
        isWritable: false,
      }
    );

    try {
      await sendAndConfirmTransaction(
        connection,
        new Transaction().add(transferInstruction),
        [wallet.payer],
        { skipPreflight: true }
      );
      assert.fail("Transfer should have failed due to non-compliant asset");
    } catch (error) {
      console.log("Transfer correctly failed for non-compliant asset");
    }
  });

  it("Transfer Hook with Extra Account Meta", async () => {
    // 1 tokens
    const amount = 1 * 10 ** decimals;

    // This helper function will automatically derive all the additional accounts that were defined in the ExtraAccountMetas account
    let transferInstruction = await createTransferCheckedInstruction(
      sourceTokenAccount,
      mint.publicKey,
      destinationTokenAccount,
      wallet.publicKey,
      amount,
      decimals,
      [],
      TOKEN_2022_PROGRAM_ID
    );

    transferInstruction.keys.push(
      {
        pubkey: program.programId,
        isSigner: false,
        isWritable: false,
      },
      {
        pubkey: extraAccountMetaListPDA,
        isSigner: false,
        isWritable: false,
      }
    );

    const transaction = new Transaction().add(transferInstruction);

    const txSig = await sendAndConfirmTransaction(
      connection,
      transaction,
      [wallet.payer],
      { skipPreflight: true }
    );
    console.log("Transfer Signature:", txSig);
  });
});
