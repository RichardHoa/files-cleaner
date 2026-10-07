import assert from "node:assert/strict";
import { test } from "node:test";
import { defaultDirection, matchesSearch, parseSearch, sortRows } from "../src/scan-query.js";

function row({ name = "a.txt", extension = "txt", size = 0, modified = 0, created = 0, path = name } = {}) {
  return {
    entry: { name, extension, size, modified, created },
    path,
    extension: extension.toLowerCase(),
    haystack: `${name}\n${path}`.toLowerCase(),
  };
}

const search = (text, r) => matchesSearch(r, parseSearch(text));

test("plain words must all appear in the name or path", () => {
  const r = row({ name: "Report.pdf", path: "docs/2026/Report.pdf" });
  assert.ok(search("report 2026", r));
  assert.ok(search("  REPORT  ", r));
  assert.ok(!search("report 2025", r));
  assert.ok(search("", r));
});

test("a dot-word matches the extension exactly, ignoring case", () => {
  assert.ok(search(".png", row({ extension: "png" })));
  assert.ok(search(".PNG", row({ extension: "png" })));
  assert.ok(!search(".png", row({ extension: "pngx" })));
  assert.ok(!search(".pn", row({ extension: "png" })));
});

test("a dot-word does not match the name or path", () => {
  assert.ok(!search(".png", row({ name: "a.png.zip", extension: "zip", path: "x.png/a.png.zip" })));
});

test("several dot-words match any of those extensions", () => {
  assert.ok(search(".png .jpg", row({ extension: "jpg" })));
  assert.ok(search(".png .jpg", row({ extension: "png" })));
  assert.ok(!search(".png .jpg", row({ extension: "gif" })));
});

test("dot-words and plain words combine", () => {
  const r = row({ name: "1-split.pdf", extension: "pdf" });
  assert.ok(search(".pdf split", r));
  assert.ok(!search(".pdf merge", r));
  assert.ok(!search(".png split", r));
});

test("a bare dot is ignored", () => {
  assert.ok(search(".", row({ extension: "png" })));
  assert.ok(search(". .png", row({ extension: "png" })));
});

test("words that merely contain a dot stay plain words", () => {
  assert.ok(search("a.png", row({ name: "a.png", extension: "png" })));
  assert.ok(!search("b.png", row({ name: "a.png", extension: "png" })));
});

test("size and dates start largest/newest first; text columns A→Z", () => {
  assert.equal(defaultDirection("size"), "desc");
  assert.equal(defaultDirection("modified"), "desc");
  assert.equal(defaultDirection("created"), "desc");
  assert.equal(defaultDirection("name"), "asc");
  assert.equal(defaultDirection("extension"), "asc");
  assert.equal(defaultDirection("path"), "asc");
});

const names = (rows) => rows.map((r) => r.entry.name);

test("names sort naturally and ignore case", () => {
  const rows = [row({ name: "10.pdf" }), row({ name: "b.pdf" }), row({ name: "2.pdf" }), row({ name: "A.pdf" })];
  assert.deepEqual(names(sortRows(rows, "name", "asc")), ["2.pdf", "10.pdf", "A.pdf", "b.pdf"]);
  assert.deepEqual(names(sortRows(rows, "name", "desc")), ["b.pdf", "A.pdf", "10.pdf", "2.pdf"]);
});

test("sizes sort numerically", () => {
  const rows = [row({ name: "s", size: 5 }), row({ name: "l", size: 1000 }), row({ name: "m", size: 90 })];
  assert.deepEqual(names(sortRows(rows, "size", "desc")), ["l", "m", "s"]);
  assert.deepEqual(names(sortRows(rows, "size", "asc")), ["s", "m", "l"]);
});

test("unknown dates sort last in either direction", () => {
  const rows = [row({ name: "none", created: null }), row({ name: "old", created: 1 }), row({ name: "new", created: 9 })];
  assert.deepEqual(names(sortRows(rows, "created", "desc")), ["new", "old", "none"]);
  assert.deepEqual(names(sortRows(rows, "created", "asc")), ["old", "new", "none"]);
});

test("extension and path sort by their text", () => {
  const rows = [row({ name: "1", extension: "PNG", path: "b/x" }), row({ name: "2", extension: "jar", path: "a/y" })];
  assert.deepEqual(names(sortRows(rows, "extension", "asc")), ["2", "1"]);
  assert.deepEqual(names(sortRows(rows, "path", "asc")), ["2", "1"]);
});

test("sorting keeps the original order of ties and leaves the input alone", () => {
  const rows = [row({ name: "first", size: 1 }), row({ name: "second", size: 1 })];
  assert.deepEqual(names(sortRows(rows, "size", "desc")), ["first", "second"]);
  assert.deepEqual(names(sortRows(rows, "size", "asc")), ["first", "second"]);
  sortRows([...rows].reverse(), "size", "asc");
  assert.deepEqual(names(rows), ["first", "second"]);
});
