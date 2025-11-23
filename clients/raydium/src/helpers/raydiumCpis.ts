import {
  TransactionInstruction,
  PublicKey,
  AccountMeta,
} from "@solana/web3.js";

/**
 * Convert a TransactionInstruction into the pieces expected by the on-chain forwarder
 * - `programId` -> external_program account in the forwarded call
 * - `data` -> raw instruction bytes (passed as `create_lp_ix` or `burn_lp_ix`)
 * - `accountMetas` -> list used as `remainingAccounts` when calling the program
 */
export function txInstructionToForwardPayload(instr: TransactionInstruction) {
  const programId = instr.programId;
  const data = Buffer.from(instr.data);
  const accountMetas: AccountMeta[] = instr.keys.map((k) => ({
    pubkey: k.pubkey,
    isSigner: k.isSigner,
    isWritable: k.isWritable,
  }));
  return { programId, data, accountMetas };
}

/**
 * Helper to prepare payload values consumable by Anchor's `remainingAccounts`
 * when calling the on-chain forwarder.
 *
 * Example usage:
 * const { programId, data, accountMetas } = txInstructionToForwardPayload(instr);
 * const remainingAccounts = accountMetas.map(a => ({ pubkey: a.pubkey, isSigner: a.isSigner, isWritable: a.isWritable }));
 * await program.rpc.performMigration(force, [...data], null, {
 *   accounts: { market, buybackState, creator, authority, externalProgram: programId, systemProgram },
 *   remainingAccounts,
 * });
 */
export function accountMetasToRemainingAccounts(accountMetas: AccountMeta[]) {
  return accountMetas.map((a) => ({
    pubkey: a.pubkey,
    isSigner: a.isSigner,
    isWritable: a.isWritable,
  }));
}

/**
 * NOTE: Building Raydium instruction bytes is versioned and relies on Raydium SDK.
 * Below is a convenience wrapper that delegates to a Raydium SDK builder if available.
 * Keep in mind: the on-chain forwarder requires the exact account ordering that the
 * external program expects. Use SDK builders to guarantee the proper layout.
 */
export async function buildRaydiumSwapInstruction(params: {
  // The SDK-specific objects: pool keys, user accounts, etc.
  // This function is illustrative; adapt to the SDK version you use.
  poolKeys: any;
  userSource: PublicKey;
  userDestination: PublicKey;
  owner: PublicKey;
}): Promise<TransactionInstruction> {
  // Import SDK lazily so users can opt-in by installing @raydium-io/raydium-sdk
  const { buildSwapInstruction } = await import("@raydium-io/raydium-sdk");
  // This call shape depends on the SDK; refer to the SDK docs for exact params
  const instr = buildSwapInstruction({
    poolKeys: params.poolKeys,
    userSource: params.userSource,
    userDestination: params.userDestination,
    owner: params.owner,
  });
  return instr as TransactionInstruction;
}
