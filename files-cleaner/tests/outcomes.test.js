import assert from "node:assert/strict";
import { test } from "node:test";
import { describeRunProgress } from "../src/outcomes.js";

test("Run progress says how many items are done of how many, then the one it is on", () => {
  assert.equal(
    describeRunProgress({ finished: 1200, total: 3000, current: "/Users/me/Downloads/a.pdf" }),
    "Moving 1,200 of 3,000 items…\n/Users/me/Downloads/a.pdf",
  );
  assert.equal(describeRunProgress({ finished: 0, total: 1, current: "/x" }), "Moving 0 of 1 item…\n/x");
});
