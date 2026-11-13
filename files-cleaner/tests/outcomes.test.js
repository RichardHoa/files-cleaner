import assert from "node:assert/strict";
import { test } from "node:test";
import { describeRunProgress, describeWarning } from "../src/shared/outcomes.js";

test("Run progress says how many items are done of how many, then the one it is on", () => {
  assert.equal(
    describeRunProgress({ finished: 1200, total: 3000, current: "/Users/me/Downloads/a.pdf" }),
    "Moving 1,200 of 3,000 items…\n/Users/me/Downloads/a.pdf",
  );
  assert.equal(describeRunProgress({ finished: 0, total: 1, current: "/x" }), "Moving 0 of 1 item…\n/x");
});

test("The warning says a file the Trash wouldn't take stayed where it was, and says nothing when all went well", () => {
  const outcome = (noteKind) => ({ source: "/Users/me/Downloads/a.pdf", note: "No Trash on this drive", noteKind });
  const summary = { done: [outcome(null)], skipped: [outcome("trashRefused")], failed: [], unrecoverable: [] };
  assert.equal(
    describeWarning(summary, { action: "run", done: "trashed", separator: "/" }),
    "1 file couldn't go to the Trash (on a drive with no Trash or a network folder, for example), so it stayed where it was.",
  );
  const clean = { done: [outcome(null)], skipped: [], failed: [], unrecoverable: [] };
  assert.equal(describeWarning(clean, { action: "run", done: "trashed", separator: "/" }), null);
});
