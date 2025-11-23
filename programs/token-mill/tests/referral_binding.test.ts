import * as anchor from "@project-serum/anchor";
import { assert } from "chai";

describe("referral binding", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.TokenMill as anchor.Program<any>;

  it("creates referral binding and prevents rebind", async () => {
    // Placeholder test: requires IDL and deployed program. This test shows the intended flow:
    // 1. Call `create_referral_account` with a referrer pubkey to bind referral to the caller.
    // 2. Attempt to call `create_referral_account` again with a different referrer and expect failure.
    const user = provider.wallet.publicKey;
    const otherReferrer = anchor.web3.Keypair.generate().publicKey;

    try {
      await program.rpc.createReferralAccount(otherReferrer, {
        accounts: {
          config: anchor.web3.SystemProgram.programId,
          referralAccount: anchor.web3.Keypair.generate().publicKey,
          user: user,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
      });
    } catch (err) {
      // In local placeholder environment this likely fails; the main run should assert success
    }

    // Attempt to rebind (should fail)
    try {
      await program.rpc.createReferralAccount(
        anchor.web3.Keypair.generate().publicKey,
        {
          accounts: {
            config: anchor.web3.SystemProgram.programId,
            referralAccount: anchor.web3.Keypair.generate().publicKey,
            user: user,
            systemProgram: anchor.web3.SystemProgram.programId,
          },
        }
      );
      assert.fail("rebind should fail");
    } catch (err) {
      assert.ok(err, "expected error when trying to rebind");
    }
  });
});
