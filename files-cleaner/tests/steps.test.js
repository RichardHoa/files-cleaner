import assert from "node:assert/strict";
import { test } from "node:test";
import { describeScanProgress, fromDuplicateStep, stepFlow } from "../src/steps.js";

const ROOT = "/Users/me/Downloads";
const firstScan = { root: ROOT, entries: [{ name: "a.pdf" }, { name: "a copy.pdf" }] };
const duplicatePlan = { root: ROOT, operations: [{ id: "trash:a copy.pdf" }] };
const emptyPlan = { root: ROOT, operations: [] };

/**
 * A step flow over fake commands: `calls` records each command and each screen shown, in order.
 * Each scan returns a new result, so a test can tell the fresh scan from the first.
 */
function setup(plan = duplicatePlan) {
  const calls = [];
  let scans = 0;
  const api = {
    buildPlan: async () => {
      calls.push(["buildPlan"]);
      return plan;
    },
    scan: async (scanRoot, onProgress) => {
      calls.push(["scan", scanRoot]);
      onProgress({ found: 1, folders: 1, skipped: 0, current: `${scanRoot}/a.pdf` });
      scans += 1;
      return { root: scanRoot, entries: [{ name: "a.pdf" }], scan: scans };
    },
  };
  const nav = {
    plan: (result, built) => calls.push(["plan", result, built]),
    suggestedFolders: (result) => calls.push(["suggestedFolders", result]),
  };
  return { calls, steps: stepFlow(api, nav) };
}

test("Find duplicates plans the duplicate step for the scan and shows its Plan", async () => {
  const { calls, steps } = setup();

  await steps.findDuplicates(firstScan);

  assert.deepEqual(calls, [["buildPlan"], ["plan", firstScan, duplicatePlan]]);
});

test("an empty duplicate Plan is still shown, so the user can see there are no duplicates and go on", async () => {
  const { calls, steps } = setup(emptyPlan);

  await steps.findDuplicates(firstScan);
  await steps.continueAfterDuplicates(ROOT, () => {});

  assert.deepEqual(
    calls.map(([name]) => name),
    ["buildPlan", "plan", "scan", "suggestedFolders"],
  );
});

test("Skip rescans the same Scan Root, then shows Suggested Folders with the fresh scan", async () => {
  const { calls, steps } = setup();
  const progress = [];

  await steps.findDuplicates(firstScan);
  await steps.continueAfterDuplicates(firstScan.root, (p) => progress.push(p));

  assert.deepEqual(calls.slice(2), [["scan", ROOT], ["suggestedFolders", { root: ROOT, entries: [{ name: "a.pdf" }], scan: 1 }]]);
  assert.equal(progress.length, 1, "the rescan reports its progress");
});

test("Continue after the duplicate Session rescans the Scan Root it was opened with", async () => {
  const { calls, steps } = setup();
  await steps.findDuplicates(firstScan);
  // The Plan screen opens the Session review with this, and its Continue reads the Scan Root from it.
  const from = fromDuplicateStep(firstScan.root);

  await steps.continueAfterDuplicates(from.scanRoot, () => {});

  assert.deepEqual(from, { step: "duplicates", scanRoot: ROOT });
  const [, shown] = calls.find(([name]) => name === "suggestedFolders");
  assert.notEqual(shown, firstScan, "grouping never gets the scan from before the Session");
  assert.equal(shown.scan, 1);
});

test("every Continue scans again, so a later one never reuses an earlier rescan", async () => {
  const { calls, steps } = setup();

  await steps.continueAfterDuplicates(ROOT, () => {});
  await steps.continueAfterDuplicates(ROOT, () => {});

  const shown = calls.filter(([name]) => name === "suggestedFolders").map(([, result]) => result.scan);
  assert.deepEqual(shown, [1, 2]);
});

test("a failed rescan shows no next step and rejects for the screen to show", async () => {
  const calls = [];
  const steps = stepFlow(
    { buildPlan: async () => emptyPlan, scan: async () => Promise.reject({ message: "Folder not found" }) },
    { plan: () => calls.push("plan"), suggestedFolders: () => calls.push("suggestedFolders") },
  );

  await assert.rejects(steps.continueAfterDuplicates(ROOT, () => {}), { message: "Folder not found" });
  assert.deepEqual(calls, []);
});

test("scan progress says what was found and read, then the item it is on", () => {
  assert.equal(
    describeScanProgress(ROOT, { found: 1200, folders: 1, skipped: 3, current: `${ROOT}/a.pdf` }),
    `Scanning ${ROOT}…\n1,200 files found · 1 folder read · 3 protected skipped\n${ROOT}/a.pdf`,
  );
});
