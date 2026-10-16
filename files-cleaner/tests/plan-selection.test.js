import assert from "node:assert/strict";
import { test } from "node:test";
import { tickState, toggleAll, totals } from "../src/plan-selection.js";

const operations = [
  { id: "trash:a.txt", kind: "trash", size: 100 },
  { id: "trash:sub/b.txt", kind: "trash", size: 20 },
  { id: "trash:sub/c.txt", kind: "trash", size: 3 },
];
const sub = ["trash:sub/b.txt", "trash:sub/c.txt"];

test("totals count only ticked Operations, by kind, with bytes to trash", () => {
  assert.deepEqual(totals(operations, new Set(["trash:a.txt", "trash:sub/c.txt"])), {
    count: 2,
    byKind: { trash: 2 },
    trashBytes: 103,
  });
  assert.deepEqual(totals(operations, new Set()), { count: 0, byKind: {}, trashBytes: 0 });
});

test("a list is all, none or some ticked", () => {
  assert.equal(tickState(sub, new Set(["trash:sub/b.txt", "trash:sub/c.txt"])), "all");
  assert.equal(tickState(sub, new Set(["trash:a.txt"])), "none");
  assert.equal(tickState(sub, new Set(["trash:sub/c.txt"])), "some");
});

test("toggling a list ticks all of it unless all of it is ticked", () => {
  const some = new Set(["trash:a.txt", "trash:sub/c.txt"]);
  assert.deepEqual([...toggleAll(sub, some)].sort(), ["trash:a.txt", "trash:sub/b.txt", "trash:sub/c.txt"]);

  const all = new Set(["trash:a.txt", "trash:sub/b.txt", "trash:sub/c.txt"]);
  assert.deepEqual([...toggleAll(sub, all)], ["trash:a.txt"]);
  assert.equal(all.size, 3, "the given set is left unchanged");
});
