import assert from "node:assert/strict";
import { test } from "node:test";
import { skippedChips } from "../src/screens/review/skipped.js";

test("the skipped chips always name subfolders and hidden files, and the others only with a count", () => {
  const labels = (result) => skippedChips(result).map((chip) => chip.label);

  assert.deepEqual(labels({ skipped: 0, noExtension: 0, unreadable: [] }), ["Subfolders: duplicates only", "Hidden files & links"]);
  assert.deepEqual(labels({ skipped: 3, noExtension: 12, unreadable: ["/a", "/b"] }), [
    "Subfolders: duplicates only",
    "No extension 12",
    "Protected folders 3",
    "Hidden files & links",
    "Unreadable 2",
  ]);
});
