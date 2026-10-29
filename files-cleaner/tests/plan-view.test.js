import assert from "node:assert/strict";
import { test } from "node:test";
import { PAGE_SIZE, folderLabel, paging, selectedFile, visibleFiles } from "../src/plan-view.js";

/** A duplicate's Trash Operation for the `/`-separated `path`, a copy of `keeper`. */
function dup(path, size = 0, keeper = "keep.txt") {
  const relativePath = path.split("/");
  return {
    id: `trash:${path}`,
    kind: "trash",
    relativePath,
    destination: ["Files Cleaner Trash", "This Session", ...relativePath],
    size,
    reason: { type: "duplicate", keeper: keeper.split("/") },
  };
}

const paths = (ops) => ops.map((op) => op.relativePath.join("/"));
const view = (ops, options = {}, ticked = new Set(ops.map((op) => op.id))) =>
  paths(visibleFiles(ops, { query: "", sort: "size", show: "all", ...options }, ticked));

test("search matches the file name or its folders, ignoring case", () => {
  const ops = [dup("Photos/IMG_1.jpg"), dup("Zoom/call.mp4"), dup("notes.txt")];
  assert.deepEqual(view(ops, { query: "img" }), ["Photos/IMG_1.jpg"]);
  assert.deepEqual(view(ops, { query: "ZOOM" }), ["Zoom/call.mp4"]);
  assert.deepEqual(view(ops, { query: "zoom/call" }), ["Zoom/call.mp4"]);
  assert.deepEqual(view(ops, { query: "  " }).length, 3);
  assert.deepEqual(view(ops, { query: "nothing" }), []);
});

test("by size, biggest first, then by name", () => {
  const ops = [dup("b.txt", 10), dup("z/big.mov", 900), dup("a.txt", 10)];
  assert.deepEqual(view(ops), ["z/big.mov", "a.txt", "b.txt"]);
});

test("by name, A to Z whatever the folder, numbers in order", () => {
  const ops = [dup("z/b.txt", 1), dup("IMG 10.jpg", 1), dup("a/c.txt", 1), dup("img 9.jpg", 1)];
  assert.deepEqual(view(ops, { sort: "name" }), ["z/b.txt", "a/c.txt", "img 9.jpg", "IMG 10.jpg"]);
});

test("by folder, then biggest first within a folder; top-level files first", () => {
  const ops = [dup("b/x.txt", 1), dup("a/y.txt", 1), dup("top.txt", 1), dup("a/z.txt", 50)];
  assert.deepEqual(view(ops, { sort: "folder" }), ["top.txt", "a/z.txt", "a/y.txt", "b/x.txt"]);
});

test("Show picks all, only the ticked or only the unticked files", () => {
  const ops = [dup("a.txt", 3), dup("b.txt", 2), dup("c.txt", 1)];
  const ticked = new Set(["trash:a.txt", "trash:c.txt"]);
  assert.deepEqual(view(ops, { show: "all" }, ticked), ["a.txt", "b.txt", "c.txt"]);
  assert.deepEqual(view(ops, { show: "ticked" }, ticked), ["a.txt", "c.txt"]);
  assert.deepEqual(view(ops, { show: "unticked" }, ticked), ["b.txt"]);
});

test("files come 50 to a page, with how many the next page would add", () => {
  assert.equal(PAGE_SIZE, 50);
  assert.deepEqual(paging(12, PAGE_SIZE), { shown: 12, more: 0 }, "fewer than one page");
  assert.deepEqual(paging(120, PAGE_SIZE), { shown: 50, more: 50 }, "the first page");
  assert.deepEqual(paging(120, 2 * PAGE_SIZE), { shown: 100, more: 20 }, "after Show more, the rest");
  assert.deepEqual(paging(120, Infinity), { shown: 120, more: 0 }, "Show all");
  assert.deepEqual(paging(0, PAGE_SIZE), { shown: 0, more: 0 });
});

test("the selected file stays selected while shown, else the first shown file is", () => {
  const [a, b] = [dup("a.txt"), dup("b.txt")];
  assert.equal(selectedFile([a, b], "trash:b.txt"), b);
  assert.equal(selectedFile([a, b], null), a, "opens on the first file");
  assert.equal(selectedFile([a], "trash:b.txt"), a, "filtered out");
  assert.equal(selectedFile([], "trash:a.txt"), null, "nothing matches");
});

test("a file's folder ends in a slash; a top-level file shows the Scan Root's name", () => {
  assert.equal(folderLabel(dup("Photos/2025/a.jpg"), "Downloads"), "Photos/2025/");
  assert.equal(folderLabel(dup("a.jpg"), "Downloads"), "Downloads/");
});
