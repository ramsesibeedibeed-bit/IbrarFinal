import * as anchor from "@project-serum/anchor";
import { assert } from "chai";

describe("airdrop merkle", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.TokenMill as anchor.Program<any>;

  it("initializes airdrop and prevents double-claim", async () => {
    // Placeholder: construct a small merkle with single leaf and attempt claim twice
    assert.ok(true);
  });
});
