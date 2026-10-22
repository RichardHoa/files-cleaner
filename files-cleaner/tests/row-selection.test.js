import assert from "node:assert/strict";
import { test } from "node:test";
import { clickSelection } from "../src/row-selection.js";

const order = ["a", "b", "c", "d", "e"];

test("a click selects one row, Cmd-click adds another, Shift-click selects up to the last one clicked", () => {
  let selection = clickSelection(new Set(), order, null, "b");
  assert.deepEqual([...selection], ["b"]);
  selection = clickSelection(selection, order, "b", "d", { toggle: true });
  assert.deepEqual([...selection], ["b", "d"]);
  selection = clickSelection(selection, order, "d", "a", { range: true });
  assert.deepEqual([...selection], ["a", "b", "c", "d"]);
});
