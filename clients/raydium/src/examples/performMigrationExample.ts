import * as anchor from "@project-serum/anchor";
import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import {
  txInstructionToForwardPayload,
  accountMetasToRemainingAccounts,
  buildRaydiumSwapInstruction,
} from "../helpers/raydiumCpis";

/**
 * Example script showing how to: build a Raydium swap/add-lp instruction, and call
 * the on-chain forwarder `perform_migration` to run that instruction signed by the market PDA.
 *
 * Notes:
 * - You must set up an Anchor `provider` (wallet + connection) in your environment.
 * - The example assumes your Anchor client IDL is available and that you have a `program`
 *   object for the token-mill program.
 * - The exact Raydium builder usage depends on the SDK version - adapt `buildRaydiumSwapInstruction` accordingly.
 */

async function main() {
  // Anchor setup (expects ANCHOR_PROVIDER_URL and wallet env vars)
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Load your program from the workspace IDL (adjust program ID)
  const idl = await anchor.Program.fetchIdl(
    new PublicKey("REPLACE_WITH_YOUR_PROGRAM_ID"),
    provider
  );
  if (!idl) {
    console.error("Failed to fetch IDL");
    return;
  }
  const program = new anchor.Program(
    idl as any,
    new PublicKey("REPLACE_WITH_YOUR_PROGRAM_ID"),
    provider
  );

  // Build a Raydium swap instruction via SDK helper (this is SDK dependent)
  // You must populate `poolKeys` with the pool-specific account keys from Raydium.
  const poolKeys = {
    /* populate pool keys from on-chain Raydium pool */
  };
  const owner = provider.wallet.publicKey;
  // user token accounts etc. must be created/funded already
  const userSource = new PublicKey("REPLACE_USER_SOURCE_ATA");
  const userDestination = new PublicKey("REPLACE_USER_DEST_ATA");

  // Build a TransactionInstruction for Raydium (SDK helper)
  const raydiumInstr: TransactionInstruction =
    await buildRaydiumSwapInstruction({
      poolKeys,
      userSource,
      userDestination,
      owner,
    });

  // Convert to forwarder payload
  const {
    programId: externalProgramId,
    data,
    accountMetas,
  } = txInstructionToForwardPayload(raydiumInstr);
  const remainingAccounts = accountMetasToRemainingAccounts(accountMetas);

  // Prepare accounts for perform_migration (fill in real pubkeys)
  const market = new PublicKey("REPLACE_MARKET_PUBKEY");
  const buybackState = new PublicKey("REPLACE_BUYBACK_STATE_PUBKEY");
  const creator = new PublicKey("REPLACE_CREATOR_PUBKEY");
  const config = new PublicKey("REPLACE_CONFIG_PUBKEY");

  // Call perform_migration with create_lp_ix as raydium instruction bytes. Note: create_lp_ix expects a Buffer/array of bytes
  await program.rpc.performMigration(
    false, // force
    Array.from(data), // create_lp_ix
    null, // burn_lp_ix
    {
      accounts: {
        market,
        buybackState,
        creator,
        authority: provider.wallet.publicKey,
        externalProgram: externalProgramId,
        config,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      remainingAccounts,
      signers: [],
    }
  );

  console.log("perform_migration invoked with forwarded Raydium instruction");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
