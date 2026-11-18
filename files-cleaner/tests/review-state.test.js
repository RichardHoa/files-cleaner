import assert from "node:assert/strict";
import { test } from "node:test";
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

/** What a Session that trashed `paths` did. */
const trashed = (paths) => ({ sessionId: "s1", done: paths.map((source) => ({ source })), unrecoverable: [] });

test("merging a folder moves its files into the other, which takes over what its trashes did", () => {
  const state = twoFolders();
  const [ielts, labs] = state.choices;
  state.setFile(ielts, "/d/a.pdf", "trash");
  const summary = trashed(["/d/a.pdf"]);
  state.ranTrashes(ielts, summary);

  state.merge(ielts, labs);

  assert.deepEqual(state.choices, [labs]);
  assert.deepEqual([...labs.files.keys()], ["/d/c.pdf", "/d/a.pdf", "/d/b.pdf"]);
  assert.deepEqual(state.trashesDone.get(labs), [summary]);
});

test("removing a folder leaves its files where they are, except those it already trashed", () => {
  const state = twoFolders();
  const [ielts] = state.choices;
  state.setFile(ielts, "/d/a.pdf", "trash");
  const summary = trashed(["/d/a.pdf"]);
  state.ranTrashes(ielts, summary);

  state.remove(ielts);

  assert.deepEqual([...state.left], ["/d/b.pdf"]);
  assert.deepEqual(state.trashesDone.get("left"), [summary]);
});
