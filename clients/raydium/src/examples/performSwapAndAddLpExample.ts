import * as anchor from "@project-serum/anchor";
import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import fs from "fs";
import path from "path";
import { txInstructionToForwardPayload, accountMetasToRemainingAccounts, buildRaydiumSwapInstruction } from "../helpers/raydiumCpis";

// This example expects a JSON file `clients/raydium/config/pool.json` containing pool keys.
// The structure should be provided by Raydium docs / SDK and include the programId and pool-specific accounts.

async function loadPoolConfig() {
  const cfgPath = path.join(__dirname, "..", "config", "pool.json");
  if (!fs.existsSync(cfgPath)) throw new Error(`Missing pool config at ${cfgPath}`);
  return JSON.parse(fs.readFileSync(cfgPath, "utf8"));
}

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idl = await anchor.Program.fetchIdl(new PublicKey(process.env.TOKEN_MILL_PROGRAM_ID || ""), provider);
  if (!idl) throw new Error("Failed to fetch IDL; set TOKEN_MILL_PROGRAM_ID in env");
  const program = new anchor.Program(idl as any, new PublicKey(process.env.TOKEN_MILL_PROGRAM_ID || ""), provider);

  const poolCfg = await loadPoolConfig();
  // Build a Raydium swap instruction using the helper (delegates to SDK).
  // You must ensure the SDK is installed and the poolCfg contains correct fields.
  const owner = provider.wallet.publicKey;
  const swapInstr: TransactionInstruction = await buildRaydiumSwapInstruction({
    poolKeys: poolCfg.poolKeys,
    userSource: new PublicKey(poolCfg.userSource),
    userDestination: new PublicKey(poolCfg.userDestination),
    owner,
  });

  // Also optionally build an add-liquidity instruction. If your SDK exposes a builder, call it here.
  // For demonstration, reuse the swap instruction as the payload to forward.
  const { programId: externalProgramId, data, accountMetas } = txInstructionToForwardPayload(swapInstr);
  const remainingAccounts = accountMetasToRemainingAccounts(accountMetas);

  // Prepare accounts for calling perform_migration or buyback depending on your flow
  const market = new PublicKey(process.env.MARKET_PUBKEY || "");
  const buybackState = new PublicKey(process.env.BUYBACK_STATE_PUBKEY || "");
  const creator = new PublicKey(process.env.CREATOR_PUBKEY || "");
  const config = new PublicKey(process.env.CONFIG_PUBKEY || "");

  // Use perform_migration as an example forwarder call (create LP)
  await program.rpc.performMigration(
    false,
    Array.from(data),
    null,
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
    }
  );

  console.log("Forwarded Raydium swap instruction to perform_migration");
}

main().catch(err => { console.error(err); process.exit(1); });
