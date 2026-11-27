import assert from "node:assert/strict";
import { test } from "node:test";
import { describeRunProgress, describeWarning } from "../src/shared/outcomes.js";

test("Run progress says how many are done of how many, then the time left once there is a guess", () => {
  const words = { verb: "Trashing", of: (n) => `${n.toLocaleString("en")} copies` };
  assert.equal(describeRunProgress({ finished: 312, total: 2040 }, words), "Trashing 312 of 2,040 copies");
  assert.equal(describeRunProgress({ finished: 312, total: 2040 }, words, "about 3 min left"), "Trashing 312 of 2,040 copies · about 3 min left");
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
