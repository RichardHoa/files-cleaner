import assert from "node:assert/strict";
import { test } from "node:test";
import { addSample, timeLeft } from "../src/shared/format.js";

/** One Operation every 100 ms from `startedAt` 0. */
const steady = (upTo) => Array.from({ length: upTo + 1 }, (_, finished) => ({ at: finished * 100, finished }));

test("no time left is shown before about 3 s, then about N min from the recent rate", () => {
  assert.equal(timeLeft(0, steady(20), 3000), "");
  assert.equal(timeLeft(0, steady(29), 3000), "");
  // 2,940 left at 10 a second is 294 s.
  assert.equal(timeLeft(0, steady(60), 3000), "about 5 min left");
});

test("the rate follows the last Operations, not the whole Run", () => {
  let samples = [];
  for (const sample of steady(100)) samples = addSample(samples, sample);
  // Ten times faster from here on.
  for (let i = 1; i <= 100; i += 1) samples = addSample(samples, { at: 10_000 + i * 10, finished: 100 + i });
  // The whole Run's rate would say about 6 min.
  assert.equal(timeLeft(0, samples, 6200), "about 1 min left");
});
