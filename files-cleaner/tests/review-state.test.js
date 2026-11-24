import assert from "node:assert/strict";
import { test } from "node:test";
import { folderTally, planChoices, planDuplicates } from "../src/screens/review/review-choices.js";
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

test("moving files skips one marked Trash, which keeps its mark where it is", () => {
  const state = twoFolders();
  const [ielts, labs] = state.choices;
  state.setFile(ielts, "/d/a.pdf", "trash");

  state.move(["/d/a.pdf", "/d/b.pdf"], labs);

  assert.deepEqual([...ielts.files], [["/d/a.pdf", "trash"]]);
  assert.deepEqual([...labs.files.keys()], ["/d/c.pdf", "/d/b.pdf"]);
});

test("leaving a folder's files unsorted takes only those not marked Trash, and the folder stays with the rest", () => {
  const state = twoFolders();
  const [ielts, labs] = state.choices;
  state.setFile(ielts, "/d/a.pdf", "trash");

  state.leaveUnsorted(ielts);
  state.leaveUnsorted(labs);

  assert.deepEqual([...state.left], ["/d/b.pdf", "/d/c.pdf"]);
  assert.deepEqual(state.choices, [ielts]);
  assert.deepEqual(folderTally(ielts), { going: 0, trashed: true });
});

test("trashing every file of a folder grays it out in the sidebar and plans each file as a Trash, with no folder made", () => {
  const state = twoFolders();
  const [ielts] = state.choices;

  for (const path of ielts.files.keys()) state.setFile(ielts, path, "trash");

  assert.deepEqual(folderTally(ielts), { going: 0, trashed: true });
  assert.deepEqual(planChoices(state.choices, state.left)[0], {
    name: "IELTS",
    choice: { type: "trashAll" },
    files: [
      ["/d/a.pdf", "trash"],
      ["/d/b.pdf", "trash"],
    ],
  });

  for (const path of ielts.files.keys()) state.setFile(ielts, path, "inFolder");
  assert.deepEqual(folderTally(ielts), { going: 2, trashed: false });
});

test("the folders read later join the Rules' folders shown first, keeping what the user did meanwhile", () => {
  const paths = ["/d/trip.jpg", "/d/plan.pdf", "/d/a.pdf", "/d/b.pdf"];
  const state = new ReviewState({ root: "/d", entries: paths.map((path) => ({ path, size: 1, modified: 0 })) }, []);
  const rule = { groupBy: { type: "folder", name: "Trip" } };
  const trip = { name: "Trip", files: ["/d/trip.jpg"], existing: null, basis: "rule", rule };
  state.showRuled({ folders: [trip], ungrouped: ["/d/plan.pdf", "/d/a.pdf", "/d/b.pdf"], summaries: {}, kinds: {} });
  const [ruled] = state.choices;
  state.rename(ruled, "Da Lat");
  state.trashLeft("/d/b.pdf", true);

  const added = state.ready({
    folders: [
      { ...trip, files: ["/d/trip.jpg", "/d/plan.pdf"] },
      { name: "IELTS", files: ["/d/a.pdf", "/d/b.pdf"], existing: null, basis: "topic" },
    ],
    ungrouped: [],
    summaries: {},
    kinds: {},
  });

  assert.equal(state.more, null);
  assert.deepEqual(planChoices(state.choices, state.left), [
    { name: "Da Lat", choice: { type: "createHere" }, files: [["/d/trip.jpg", "inFolder"], ["/d/plan.pdf", "inFolder"]] },
    { name: "IELTS", choice: { type: "createHere" }, files: [["/d/a.pdf", "inFolder"]] },
    { name: "Left where they are", choice: { type: "trashAll" }, files: [["/d/b.pdf", "inFolder"]] },
  ]);
  assert.deepEqual(added, [state.choices[1]]);
});

test("copies trashed on their own leave the Plan, and a group left with one copy is a file like any other", () => {
  const paths = ["/d/a.pdf", "/d/x/a.pdf"];
  const group = { size: 1, copies: paths.map((path) => ({ path, modified: 0 })) };
  const state = new ReviewState({ root: "/d", entries: paths.map((path) => ({ path, size: 1, modified: 0 })) }, [group]);
  state.ready({ folders: [{ name: "Docs", files: ["/d/a.pdf"], existing: null, basis: "topic" }], ungrouped: [], summaries: {}, kinds: {} });

  state.copiesTrashed(["/d/x/a.pdf"]);

  assert.deepEqual(state.duplicates, []);
  assert.deepEqual(planChoices(state.choices, state.left), [{ name: "Docs", choice: { type: "createHere" }, files: [["/d/a.pdf", "inFolder"]] }]);
});

test("a Suggested Folder trashed now trashes exactly its listed files, then leaves the Plan while the others stay", () => {
  const state = twoFolders();
  const [ielts] = state.choices;

  const { folders, trashed } = state.trashNow([ielts]);
  assert.deepEqual(folders, [
    {
      name: "IELTS",
      choice: { type: "trashAll" },
      files: [
        ["/d/a.pdf", "trash"],
        ["/d/b.pdf", "trash"],
      ],
    },
  ]);

  trashed(["/d/a.pdf", "/d/b.pdf"]);
  assert.deepEqual(planChoices(state.choices, state.left), [{ name: "Labs", choice: { type: "createHere" }, files: [["/d/c.pdf", "inFolder"]] }]);
});

test("a folder trashed now with a Duplicate Group's Filed Copy in it leaves that group another copy Kept", () => {
  const paths = ["/d/a.pdf", "/d/x/a.pdf", "/d/y/a.pdf"];
  const group = { size: 1, copies: paths.map((path) => ({ path, modified: 0 })) };
  const state = new ReviewState({ root: "/d", entries: paths.map((path) => ({ path, size: 1, modified: 0 })) }, [group]);
  state.ready({ folders: [{ name: "Docs", files: ["/d/a.pdf"], existing: null, basis: "topic" }], ungrouped: [], summaries: {}, kinds: {} });
  const [docs] = state.choices;

  state.trashNow([docs]).trashed(["/d/a.pdf"]);

  assert.deepEqual(planDuplicates(state.duplicates), [{ kept: "/d/x/a.pdf", trash: ["/d/y/a.pdf"] }]);
  assert.deepEqual([...state.left], ["/d/x/a.pdf"]);
  assert.deepEqual(state.choices, []);
});
