import assert from "node:assert/strict";
import { test } from "node:test";
import { FOLDER_LEVELS, nearestLevel } from "../src/screens/settings/grouping-levels.js";

test("The middle level is today's default, and a stored value between levels snaps to the nearest", () => {
  assert.equal(FOLDER_LEVELS[2].threshold, 0.86);
  assert.equal(FOLDER_LEVELS[nearestLevel(FOLDER_LEVELS, 0.91)].threshold, 0.92);
});
