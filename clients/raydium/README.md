Raydium helper client

This folder includes TypeScript helpers and an example showing how to construct Raydium (or other DEX) TransactionInstructions and forward them to the on-chain forwarder in the Token Mill program.

Setup

1. Install dependencies (run from this folder):

```bash
npm install
```

2. Build TypeScript:

```bash
npm run build
```

3. Edit `src/examples/performMigrationExample.ts` replacing the placeholder pubkeys and pool keys with real values.

4. Run the example (after build):

```bash
npm run example
```

Notes & guidance

- The on-chain forwarder expects:

  - `external_program` account to match the instruction's `programId`.
  - `create_lp_ix` / `burn_lp_ix` to be the raw instruction `data` bytes (array of u8).
  - `remainingAccounts` to match the instruction `keys` (order-sensitive).

- Use SDK builders (for Raydium) to guarantee correct account order and instruction encoding. The `buildRaydiumSwapInstruction` helper uses `@raydium-io/raydium-sdk` if installed.

- Security: ensure the `TokenMillConfig.cpi_whitelist` includes Raydium program ID and `max_forwarded_accounts` is set to a safe value before calling the forwarder.

- This client is illustrative. Adapt the helper calls to match the exact SDK and program versions used in your environment.
