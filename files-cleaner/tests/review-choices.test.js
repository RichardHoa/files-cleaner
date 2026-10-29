import assert from "node:assert/strict";
import { test } from "node:test";
import { byKindNewestFirst, initialChoices, moveFile, planChoices } from "../src/review-choices.js";

test("a file dragged to another folder leaves the first, and an emptied folder isn't planned", () => {
  const choices = initialChoices([
    { name: "IELTS", files: ["/d/a.pdf"], existing: null },
    { name: "Labs", files: ["/d/b.pdf"], existing: null },
  ]);
  const left = new Set(["/d/c.pdf"]);
  moveFile(choices, left, "/d/a.pdf", choices[1]);
  moveFile(choices, left, "/d/c.pdf", choices[1]);
  assert.deepEqual(planChoices(choices), [
    {
      name: "Labs",
      choice: { type: "createHere" },
      files: [
        ["/d/b.pdf", "inFolder"],
        ["/d/a.pdf", "inFolder"],
        ["/d/c.pdf", "inFolder"],
      ],
    },
  ]);
  assert.equal(left.size, 0);
});

test("files left where they are go by kind, Other last, each kind newest first", () => {
  const kinds = { "a.pdf": "Documents", "b.html": "Code", "c.pdf": "Documents", "d.xyz": "Other" };
  const dates = { "a.pdf": 1, "b.html": 5, "c.pdf": 3, "d.xyz": 9 };
  const sorted = byKindNewestFirst(Object.keys(kinds), (p) => kinds[p], (p) => dates[p]);
  assert.deepEqual(sorted, [
    ["Code", ["b.html"]],
    ["Documents", ["c.pdf", "a.pdf"]],
    ["Other", ["d.xyz"]],
  ]);
});
