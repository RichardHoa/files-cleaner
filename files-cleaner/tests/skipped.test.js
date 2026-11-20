import assert from "node:assert/strict";
import { test } from "node:test";
import { skippedLine } from "../src/screens/review/skipped.js";

test("the skipped line shows only when the scan skipped something, with each count in its detail", () => {
  assert.equal(skippedLine({ skipped: 0, noExtension: 0, unreadable: [] }), null);

  const line = skippedLine({ skipped: 3, noExtension: 12, unreadable: ["/a", "/b"] });
  assert.equal(line.label, "Some files skipped");
  assert.match(line.detail, /Files without an extension: 12/);
  assert.match(line.detail, /Protected folders: 3/);
  assert.match(line.detail, /Unreadable: 2/);
});
