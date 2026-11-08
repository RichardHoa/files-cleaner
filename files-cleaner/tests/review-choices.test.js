import assert from "node:assert/strict";
import { test } from "node:test";
import {
  byKindNewestFirst,
  describeChange,
  folderTally,
  foldersHoldingCopies,
  initialChoices,
  initialDuplicates,
  LeftFiles,
  mergeFolder,
  moveFile,
  moveToOtherFolder,
  planChoices,
  planDuplicates,
  removeFolder,
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

test("merging moves every file into the target, which keeps its name, and drops the merged folder", () => {
  const choices = initialChoices([
    { name: "IELTS", files: ["/d/a.pdf", "/d/b.pdf"], existing: null },
    { name: "Labs", files: ["/d/c.pdf"], existing: null },
  ]);
  const [ielts, labs] = choices;
  ielts.files.set("/d/b.pdf", "trash");
  mergeFolder(choices, [], ielts, labs);
  assert.deepEqual(planChoices(choices), [
    {
      name: "Labs",
      choice: { type: "createHere" },
      files: [
        ["/d/c.pdf", "inFolder"],
        ["/d/a.pdf", "inFolder"],
        ["/d/b.pdf", "trash"],
      ],
    },
  ]);
});

test("removing a folder leaves its files where they are, and a group with no copy kept comes back there too", () => {
  const { duplicate, choices, left } = threeCopies();
  setCopy(duplicate, "/d/a.pdf", "trash", choices, left);
  removeFolder(choices, left, [duplicate], choices[0]);
  assert.deepEqual(choices, []);
  assert.deepEqual([...left], ["/d/b.pdf"]);

  setCopy(duplicate, "/d/x/a.pdf", "keep", choices, left);
  assert.deepEqual([...left], ["/d/b.pdf", "/d/x/a.pdf"]);
});

test("files moved to another folder end up there in the Plan, and one already in it stays where it is", () => {
  const choices = initialChoices([{ name: "Labs", files: ["/d/a.pdf"], existing: null }]);
  const left = new LeftFiles(["/d/b.pdf", "/other/c.pdf"]);

  moveToOtherFolder(choices, left, ["/d/a.pdf", "/other/c.pdf"], "/other", "/");
  moveToOtherFolder(choices, left, ["/d/b.pdf"], "/other", "/");

  assert.deepEqual(planChoices(choices, left), [
    {
      name: "other",
      choice: { type: "addTo", folder: "/other" },
      files: [
        ["/d/a.pdf", "inFolder"],
        ["/d/b.pdf", "inFolder"],
      ],
    },
  ]);
  assert.deepEqual([...left], ["/other/c.pdf"]);
});

test("a file left where it is can be trashed, until it leaves", () => {
  const choices = initialChoices([{ name: "Labs", files: [], existing: null }]);
  const left = new LeftFiles(["/d/a.pdf", "/d/b.pdf"]);
  left.trashed.add("/d/a.pdf");
  left.trashed.add("/d/b.pdf");

  moveFile(choices, left, "/d/b.pdf", choices[0]);

  assert.deepEqual(planChoices(choices, left), [
    { name: "Labs", choice: { type: "createHere" }, files: [["/d/b.pdf", "inFolder"]] },
    { name: "Left where they are", choice: { type: "trashAll" }, files: [["/d/a.pdf", "inFolder"]] },
  ]);
});

test("the count leaves out a file already in the folder it goes to, and takes in the files trashed from Left", () => {
  const choices = initialChoices([{ name: "Old", files: ["/d/Old/a.pdf", "/d/b.pdf"], existing: "/d/Old" }]);
  const left = new LeftFiles(["/d/c.pdf"]);
  left.trashed.add("/d/c.pdf");
  assert.deepEqual(totals(choices, left, "/"), { moved: 1, trashed: 1 });
});

test("the header and button say what will happen to the files", () => {
  assert.deepEqual(describeChange({ moved: 0, trashed: 0 }), { heading: "Nothing will change", button: "Nothing to move" });
  assert.deepEqual(describeChange({ moved: 2, trashed: 0 }), { heading: "2 files will move", button: "Move 2 files" });
  assert.deepEqual(describeChange({ moved: 0, trashed: 1 }), { heading: "1 file will be trashed", button: "Trash 1 file" });
  assert.deepEqual(describeChange({ moved: 2, trashed: 1 }), { heading: "2 files will move, 1 will be trashed", button: "Move 2, trash 1" });
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
  return { duplicate, choices, left: new LeftFiles() };
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
  assert.deepEqual(totals(choices, left, "/", [duplicate]), { moved: 2, trashed: 2 });
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

test("a folder counts only the files going into it, and is trashed once every file is", () => {
  const [folder] = initialChoices([{ name: "Labs", files: ["/d/a.pdf", "/d/b.pdf", "/d/c.pdf"], existing: null }]);
  folder.files.set("/d/a.pdf", "trash");
  folder.files.set("/d/b.pdf", "takeOut");
  assert.deepEqual(folderTally(folder), { going: 1, trashed: false });
  folder.files.set("/d/b.pdf", "trash");
  folder.files.set("/d/c.pdf", "trash");
  assert.deepEqual(folderTally(folder), { going: 0, trashed: true });
  folder.files.set("/d/c.pdf", "takeOut");
  assert.deepEqual(folderTally(folder), { going: 0, trashed: false });
  folder.files.set("/d/c.pdf", "inFolder");
  folder.choice = "trashAll";
  assert.deepEqual(folderTally(folder), { going: 0, trashed: true });
});
