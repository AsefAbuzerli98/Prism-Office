import { describe, expect, it } from "vitest";
// @ts-expect-error — plain ESM script without type declarations
import { checkCommitMessage } from "../../scripts/check-commit-msg.mjs";

describe("commit-msg hook", () => {
  it("accepts conventional headers", () => {
    for (const m of ["feat(word): add markdown view", "fix!: drop legacy api", "docs: adr 0001\n\nbody", "Merge branch 'x'"]) {
      expect(checkCommitMessage(m)).toBeNull();
    }
  });
  it("rejects others", () => {
    expect(checkCommitMessage("Phase 1 complete")).toMatch(/Conventional/);
    expect(checkCommitMessage("feat: x\nno blank line")).toMatch(/blank line/);
    expect(checkCommitMessage("feature: x")).toMatch(/Conventional/);
  });
});
