import assert from "node:assert/strict";
import { test } from "node:test";
import { readingEstimate, readingNeedsAsking, updatedAverage } from "../src/screens/review/reading-estimate.js";

test("with no history, the time left is 4 s × the files left", () => {
  assert.deepEqual(readingEstimate({ average: null, total: 30, finished: 0, elapsed: 0 }), { perFile: 4, left: 120 });
  assert.deepEqual(readingEstimate({ average: null, total: 30, finished: 2, elapsed: 8 }), { perFile: 4, left: 112 });
});

test("the countdown goes on until the folders are shown, through the last pass", () => {
  assert.deepEqual(readingEstimate({ average: 4, total: 5, finished: 5, elapsed: 15 }), { perFile: 3.5, left: 2.5 });
});

test("a finished run updates the stored average", () => {
  assert.equal(updatedAverage(null, { seconds: 100, files: 20 }), 5);
  assert.equal(updatedAverage(4, { seconds: 120, files: 20 }), 5);
});

test("reading is asked about only when it takes over 10 minutes, at 4 s a file with no history", () => {
  assert.equal(readingNeedsAsking({ average: null, toRead: 150 }), false);
  assert.equal(readingNeedsAsking({ average: null, toRead: 151 }), true);
  assert.equal(readingNeedsAsking({ average: 1, toRead: 300 }), false);
});
