import assert from "node:assert/strict";
import { test } from "node:test";
import { PAGE_SIZE, paging, selectedFile, visibleFiles } from "../src/plan-view.js";
import { groupByState, paneSentence, sessionHeading, sessionLead } from "../src/session-view.js";

/** A Session's Trash Operation for the `/`-separated `path`, in `state`. */
function op(path, state = "done", { size = 0, note = null } = {}) {
  const relativePath = path.split("/");
  return { id: `trash:${path}`, kind: "trash", inSystemTrash: true, relativePath, size, state, note };
}

test("each state goes into its group, in a fixed order, and empty groups are left out", () => {
  const ops = ["started", "undone", "failed", "done", "skipped", "unrecoverable"].map((state) => op(`${state}.txt`, state));
  const groups = groupByState(ops).map((g) => [g.group, g.operations.map((o) => o.state)]);
  assert.deepEqual(groups, [
    ["trashed", ["done"]],
    ["putBack", ["undone"]],
    ["attention", ["started", "failed", "skipped", "unrecoverable"]],
  ]);
  assert.deepEqual(
    groupByState([op("a.txt", "undone")]).map((g) => g.group),
    ["putBack"],
  );
  assert.deepEqual(groupByState([]), []);
});

const many = (n, state, prefix = state) => Array.from({ length: n }, (_, i) => op(`${prefix}/${i}.txt`, state));

test("the heading says what moved, and keeps up as files are put back", () => {
  assert.equal(sessionHeading([...many(3, "done"), op("x.txt", "skipped")], "completed"), "3 trashed");
  assert.equal(sessionHeading([...many(2, "done"), ...many(1, "undone")], "partlyUndone"), "1 of 3 files put back");
  assert.equal(sessionHeading(many(3, "undone"), "undone"), "All 3 files are back");
});

test("a Session that stopped part-way says how far it got", () => {
  const ops = [...many(2, "done"), op("x.txt", "skipped"), op("y.txt", "started")];
  assert.equal(sessionHeading(ops, "crashed"), "Stopped after 2 of 4 files");
});

test("one file is singular", () => {
  assert.equal(sessionHeading([op("a.txt")], "completed"), "1 trashed");
  assert.equal(sessionHeading([op("a.txt", "undone")], "undone"), "All 1 file is back");
  assert.equal(sessionHeading([op("a.txt", "started")], "crashed"), "Stopped after 0 of 1 file");
});

test("the pane says where a moved file is, and where a put-back one is", () => {
  assert.equal(paneSentence(op("Zoom/a.mp4"), "Downloads", "/"), "In the Trash.");
  assert.equal(paneSentence(op("Zoom/a.mp4"), "Downloads", "\\"), "In the Recycle Bin.");
  assert.equal(paneSentence(op("Zoom/a.mp4", "undone"), "Downloads"), "Back in Zoom/.");
  assert.equal(paneSentence(op("a.mp4", "undone"), "Downloads"), "Back in Downloads/.");
});

test("the pane adds why the last Put back or Move again didn't happen", () => {
  const note = "Zoom/a.mp4 already exists";
  assert.equal(
    paneSentence(op("Zoom/a.mp4", "done", { note }), "Downloads", "/"),
    "In the Trash. Not put back: Zoom/a.mp4 already exists",
  );
  assert.equal(paneSentence(op("Zoom/a.mp4", "undone", { note }), "Downloads"), "Back in Zoom/. Not moved again: Zoom/a.mp4 already exists");
});

test("under Needs attention, the pane says what happened, and why when it is known", () => {
  const note = "It no longer exists";
  assert.equal(paneSentence(op("a.txt", "skipped", { note }), "Downloads"), "Skipped: It no longer exists");
  assert.equal(paneSentence(op("a.txt", "failed", { note }), "Downloads"), "Failed: It no longer exists");
  assert.equal(paneSentence(op("a.txt", "unrecoverable", { note }), "Downloads"), "Couldn't be recovered: It no longer exists");
  assert.equal(paneSentence(op("a.txt", "started"), "Downloads"), "Stopped before this file finished.");
  assert.equal(paneSentence(op("a.txt", "skipped"), "Downloads"), "Skipped.");
});

const paths = (ops) => ops.map((o) => o.relativePath.join("/"));

test("a group's files are searched and sorted with the Plan's functions, with no Show filter", () => {
  const ops = [op("Zoom/call.mp4", "done", { size: 5 }), op("Photos/IMG_1.jpg", "done", { size: 900 }), op("notes.txt", "done", { size: 5 })];
  assert.deepEqual(paths(visibleFiles(ops, { query: "", sort: "size" })), ["Photos/IMG_1.jpg", "Zoom/call.mp4", "notes.txt"]);
  assert.deepEqual(paths(visibleFiles(ops, { query: "zoom", sort: "size" })), ["Zoom/call.mp4"]);
  assert.deepEqual(paths(visibleFiles(ops, { query: "", sort: "folder" })), ["notes.txt", "Photos/IMG_1.jpg", "Zoom/call.mp4"]);
  assert.deepEqual(visibleFiles(ops, { query: "nothing", sort: "name" }), []);
});

test("a long group is paged with the Plan's paging", () => {
  const ops = many(PAGE_SIZE * 2 + 3, "done");
  const files = visibleFiles(ops, { query: "", sort: "name" });
  assert.deepEqual(paging(files.length, PAGE_SIZE), { shown: PAGE_SIZE, more: PAGE_SIZE });
  assert.deepEqual(paging(files.length, PAGE_SIZE * 2), { shown: PAGE_SIZE * 2, more: 3 });
});

test("when search hides the selected file, the first file still shown is selected", () => {
  const ops = [op("Zoom/call.mp4"), op("Zoom/notes.txt"), op("other.pdf")];
  const shown = visibleFiles(ops, { query: "zoom", sort: "name" });
  assert.equal(selectedFile(shown, "trash:other.pdf"), shown[0]);
  assert.equal(selectedFile(shown, "trash:Zoom/notes.txt").id, "trash:Zoom/notes.txt");
});

/** A Session's Operation of `kind` for `path`, which went to the absolute `destination`. */
function ruleOp(kind, path, destination, state = "done") {
  const relativePath = path.split("/");
  return { id: `${kind}:${path}`, kind, relativePath, destination, size: 0, state, note: null };
}

test("an older Session's Trash is named by the folder it went to, not the System Trash", () => {
  const old = { ...ruleOp("trash", "a.mp4", "/r/Files Cleaner Trash/S/a.mp4"), inSystemTrash: false };
  assert.equal(paneSentence(old, "r", "/"), "In /r/Files Cleaner Trash/S.");
  assert.equal(sessionHeading([old], "completed"), "1 moved");
});

test("the lead says files can be put back until the System Trash is emptied, if any went there", () => {
  assert.match(sessionLead([op("a.txt")], "/"), /until you empty the Trash\.$/);
  assert.match(sessionLead([ruleOp("move", "a.pptx", "/r/a.pptx")], "/"), /at any time\.$/);
});

test("the pane says where a file a Rule moved is, and what happened to a created folder", () => {
  assert.equal(paneSentence(ruleOp("move", "a.pptx", "/Users/me/Study/a.pptx"), "Downloads", "/"), "In /Users/me/Study.");
  assert.equal(paneSentence(ruleOp("move", "a.pptx", "C:\\Study\\a.pptx"), "Downloads", "\\"), "In C:\\Study.");
  assert.equal(paneSentence(ruleOp("createFolder", "Study", "/r/Study"), "Downloads"), "Created by this Session.");
  assert.equal(paneSentence(ruleOp("createFolder", "Study", "/r/Study", "undone"), "Downloads"), "Removed again.");
});

/** A Session's Move Operation of `path` into `Sorted/`, in `state`. */
const moveOp = (path, state = "done") => ({ ...op(path, state), id: `move:${path}`, kind: "move", inSystemTrash: false, destination: `/d/Sorted/${path}` });

test("moved and trashed files are grouped apart, and the heading counts each", () => {
  const ops = [moveOp("a.pdf"), moveOp("b.pdf"), op("c.zip"), { ...moveOp("Sorted"), kind: "createFolder" }];
  assert.deepEqual(
    groupByState(ops).map((g) => [g.group, g.operations.length]),
    [
      ["moved", 3],
      ["trashed", 1],
    ],
  );
  assert.equal(sessionHeading(ops, "completed"), "2 moved, 1 trashed, 1 folder created");
  assert.equal(sessionHeading([op("c.zip"), op("d.zip")], "completed"), "2 trashed");
});
