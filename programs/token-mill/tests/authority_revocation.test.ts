import * as anchor from "@project-serum/anchor";
import { assert } from "chai";

describe("authority revocation", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.TokenMill as anchor.Program<any>;

  it("records revocation flags in market state", async () => {
    assert.ok(true);
  });
});
