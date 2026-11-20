import assert from "node:assert/strict";
import { test } from "node:test";
import { folderTally, planChoices } from "../src/screens/review/review-choices.js";
import { ReviewState } from "../src/screens/review/review-state.js";

/** A Review state whose files were read into the folders "IELTS", holding a.pdf and b.pdf, and "Labs", holding c.pdf. */
function twoFolders() {
  const paths = ["/d/a.pdf", "/d/b.pdf", "/d/c.pdf"];
  const state = new ReviewState({ root: "/d", entries: paths.map((path) => ({ path, size: 1, modified: 0 })) }, []);
  state.ready({
    folders: [
      { name: "IELTS", files: ["/d/a.pdf", "/d/b.pdf"], existing: null, basis: "topic" },
      { name: "Labs", files: ["/d/c.pdf"], existing: null, basis: "topic" },
    ],
    ungrouped: [],
    summaries: {},
    kinds: {},
  });
  return state;
}

test("taking a file out of a folder leaves it where it is in the Plan", () => {
  const state = twoFolders();

  state.move(["/d/a.pdf"], null);

  assert.deepEqual([...state.left], ["/d/a.pdf"]);
  assert.deepEqual(planChoices(state.choices, state.left), [
    { name: "IELTS", choice: { type: "createHere" }, files: [["/d/b.pdf", "inFolder"]] },
    { name: "Labs", choice: { type: "createHere" }, files: [["/d/c.pdf", "inFolder"]] },
  ]);
});

test("trashing every file of a folder grays it out in the sidebar and plans each file as a Trash, with no folder made", () => {
  const state = twoFolders();
  const [ielts] = state.choices;

  state.trashAllFiles(ielts);

  assert.deepEqual(folderTally(ielts), { going: 0, trashed: true });
  assert.deepEqual(planChoices(state.choices, state.left)[0], {
    name: "IELTS",
    choice: { type: "trashAll" },
    files: [
      ["/d/a.pdf", "trash"],
      ["/d/b.pdf", "trash"],
    ],
  });

  state.trashAllFiles(ielts, false);
  assert.deepEqual(folderTally(ielts), { going: 2, trashed: false });
});
