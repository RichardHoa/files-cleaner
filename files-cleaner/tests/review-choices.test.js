import assert from "node:assert/strict";
import { test } from "node:test";
import {
  byKindNewestFirst,
  foldersHoldingCopies,
  initialChoices,
  initialDuplicates,
  moveFile,
  planChoices,
  planDuplicates,
  setCopiesIn,
  setCopy,
  totals,
} from "../src/review-choices.js";

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

/** A Duplicate Group of three copies, the first read into the folder "Labs", with nothing left where it is. */
function threeCopies() {
  const [duplicate] = initialDuplicates([{ size: 3, copies: [{ path: "/d/a.pdf" }, { path: "/d/x/a.pdf" }, { path: "/d/y/a.pdf" }] }]);
  const choices = initialChoices([{ name: "Labs", files: ["/d/b.pdf", "/d/a.pdf"], existing: null }]);
  return { duplicate, choices, left: new Set() };
}

test("keeping two copies files the first and lists the second under Left; trashing the Filed Copy hands its place to the next", () => {
  const { duplicate, choices, left } = threeCopies();
  setCopy(duplicate, "/d/x/a.pdf", "keep", choices, left);
  assert.deepEqual(planDuplicates([duplicate]), [{ kept: "/d/a.pdf", trash: ["/d/y/a.pdf"] }]);
  assert.deepEqual(planChoices(choices)[0].files, [
    ["/d/b.pdf", "inFolder"],
    ["/d/a.pdf", "inFolder"],
  ]);
  assert.deepEqual([...left], ["/d/x/a.pdf"]);

  setCopy(duplicate, "/d/a.pdf", "trash", choices, left);
  assert.deepEqual(planDuplicates([duplicate]), [{ kept: "/d/x/a.pdf", trash: ["/d/a.pdf", "/d/y/a.pdf"] }]);
  assert.deepEqual(planChoices(choices)[0].files, [
    ["/d/b.pdf", "inFolder"],
    ["/d/x/a.pdf", "inFolder"],
  ]);
  assert.equal(left.size, 0);
  assert.deepEqual(totals(choices, [duplicate]), { moved: 2, trashed: 2 });
});

test("a copy kept is never trashed because the copy before it was marked Trash in its folder", () => {
  const { duplicate, choices, left } = threeCopies();
  setCopy(duplicate, "/d/x/a.pdf", "keep", choices, left);
  choices[0].files.set("/d/a.pdf", "trash");
  setCopy(duplicate, "/d/a.pdf", "trash", choices, left);
  assert.equal(choices[0].files.get("/d/x/a.pdf"), "inFolder");
});

/** A Duplicate Group of the file `name`, with a copy in each of `folders`, "" being the Scan Root. */
const groupIn = (name, folders) => ({
  size: 1,
  copies: folders.map((folder) => ({
    path: `/d/${folder ? `${folder}/` : ""}${name}`,
    relativePath: [...(folder ? folder.split("/") : []), name],
  })),
});

test("the folders holding copies are listed once each, A-Z, the Scan Root first", () => {
  const duplicates = initialDuplicates([groupIn("a.pdf", ["y", "x/old"]), groupIn("b.pdf", ["", "y"])]);
  assert.deepEqual(foldersHoldingCopies(duplicates), ["", "x/old", "y"]);
});

test("trashing every copy in a folder trashes its copies in untouched groups only", () => {
  const [untouched, byHand] = initialDuplicates([groupIn("a.pdf", ["x", "y"]), groupIn("b.pdf", ["x", "y"])]);
  const left = new Set();
  setCopy(byHand, "/d/y/b.pdf", "keep", [], left);

  setCopiesIn([untouched, byHand], "x", "trash", [], left);

  assert.equal(untouched.copies.get("/d/x/a.pdf"), "trash");
  assert.deepEqual([...byHand.copies.values()], ["keep", "keep"]);
});

test("trashing every copy takes the group out of its folder, and keeping one again puts it back", () => {
  const { duplicate, choices, left } = threeCopies();
  setCopy(duplicate, "/d/a.pdf", "trash", choices, left);
  assert.deepEqual(planDuplicates([duplicate]), [{ kept: null, trash: ["/d/a.pdf", "/d/x/a.pdf", "/d/y/a.pdf"] }]);
  assert.deepEqual(planChoices(choices)[0].files, [["/d/b.pdf", "inFolder"]]);

  setCopy(duplicate, "/d/y/a.pdf", "keep", choices, left);
  assert.deepEqual(planChoices(choices)[0].files, [
    ["/d/b.pdf", "inFolder"],
    ["/d/y/a.pdf", "inFolder"],
  ]);
  assert.equal(left.size, 0);
});
