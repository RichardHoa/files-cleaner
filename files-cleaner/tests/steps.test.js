import assert from "node:assert/strict";
import { test } from "node:test";
import { describeDuplicateProgress, describeScanProgress, stepFlow } from "../src/steps.js";

const ROOT = "/Users/me/Downloads";
const groups = [{ size: 4, copies: [{ path: `${ROOT}/a.pdf` }, { path: `${ROOT}/Old/a.pdf` }] }];

/** A step flow over fake commands: `calls` records each command and each screen shown, in order. */
function setup() {
  const calls = [];
  const api = {
    scan: async (scanRoot, onProgress) => {
      calls.push(["scan", scanRoot]);
      onProgress({ found: 1, folders: 1, skipped: 0, current: `${scanRoot}/a.pdf` });
      return { root: scanRoot, entries: [{ name: "a.pdf" }] };
    },
    findDuplicates: async (onProgress) => {
      calls.push(["findDuplicates"]);
      onProgress({ phase: "comparing", finished: 0, total: 2, current: `${ROOT}/a.pdf` });
      return groups;
    },
  };
  const nav = {
    review: (result, duplicates) => calls.push(["review", result, duplicates]),
    scanResults: () => calls.push(["scanResults"]),
  };
  return { calls, steps: stepFlow(api, nav) };
}

test("picking a folder scans it once, finds its duplicates, then shows the review", async () => {
  const { calls, steps } = setup();
  const progress = [];

  await steps.scanFolder(ROOT, (line) => progress.push(line));

  assert.deepEqual(calls, [["scan", ROOT], ["findDuplicates"], ["review", { root: ROOT, entries: [{ name: "a.pdf" }] }, groups]]);
  assert.equal(progress.length, 2, "the scan and the duplicate search each report their progress");
});

test("a failed scan shows no review and rejects for the screen to show", async () => {
  const calls = [];
  const steps = stepFlow(
    { scan: async () => Promise.reject({ message: "Folder not found" }), findDuplicates: async () => groups },
    { review: () => calls.push("review") },
  );

  await assert.rejects(steps.scanFolder(ROOT, () => {}), { message: "Folder not found" });
  assert.deepEqual(calls, []);
});

test("scan progress says what was found and read, then the item it is on", () => {
  assert.equal(
    describeScanProgress(ROOT, { found: 1200, folders: 1, skipped: 3, current: `${ROOT}/a.pdf` }),
    `Scanning ${ROOT}…\n1,200 files found · 1 folder read · 3 protected skipped\n${ROOT}/a.pdf`,
  );
});

test("duplicate progress counts files while comparing, then bytes while checking, then the file it is on", () => {
  assert.equal(
    describeDuplicateProgress({ phase: "comparing", finished: 1200, total: 5000, current: `${ROOT}/a.pdf` }),
    `Comparing files of the same size: 1,200 of 5,000…\n${ROOT}/a.pdf`,
  );
  assert.equal(
    describeDuplicateProgress({ phase: "checking", finished: 1.5e9, total: 4e9, current: `${ROOT}/a.mov` }),
    `Checking files that look alike: 1.5 GB of 4.0 GB…\n${ROOT}/a.mov`,
  );
});
