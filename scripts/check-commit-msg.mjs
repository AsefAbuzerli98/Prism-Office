#!/usr/bin/env node
// Conventional Commits check: `type(scope)!: subject`, used by .githooks/commit-msg.
import { readFileSync } from "node:fs";

export const TYPES = ["feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert"];
const HEADER = new RegExp(`^(${TYPES.join("|")})(\\([a-z0-9][a-z0-9._/-]*\\))?!?: \\S.{0,99}$`);

/** Returns an error message, or null when the message is acceptable. */
export function checkCommitMessage(message) {
  const lines = message.split("\n").filter((l) => !l.startsWith("#"));
  const header = (lines[0] ?? "").trimEnd();
  if (/^(Merge|Revert|fixup!|squash!) /.test(header)) return null;
  if (!HEADER.test(header)) {
    return `Commit header must follow Conventional Commits, e.g. "feat(word): add markdown view".\n` +
      `Allowed types: ${TYPES.join(", ")}. Subject ≤ 100 chars.\nGot: "${header}"`;
  }
  if (lines.length > 1 && lines[1].trim() !== "") return "Leave a blank line between the header and the body.";
  return null;
}

if (process.argv[1] && process.argv[1].endsWith("check-commit-msg.mjs") && process.argv[2]) {
  const err = checkCommitMessage(readFileSync(process.argv[2], "utf8"));
  if (err) {
    console.error(`✖ ${err}`);
    process.exit(1);
  }
}
