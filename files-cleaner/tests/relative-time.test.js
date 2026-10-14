import assert from "node:assert/strict";
import { test } from "node:test";
import { relativeTime } from "../src/format.js";

const now = Date.UTC(2026, 9, 14, 12, 0, 0);
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

test("under a minute is just now", () => {
  assert.equal(relativeTime(now - 30_000, now), "just now");
  assert.equal(relativeTime(now, now), "just now");
});

test("a time in the future, from a clock change, is just now", () => {
  assert.equal(relativeTime(now + HOUR, now), "just now");
});

test("minutes, hours, days, months and years, singular and plural", () => {
  assert.equal(relativeTime(now - MINUTE, now), "1 minute ago");
  assert.equal(relativeTime(now - 59 * MINUTE, now), "59 minutes ago");
  assert.equal(relativeTime(now - HOUR, now), "1 hour ago");
  assert.equal(relativeTime(now - 23 * HOUR, now), "23 hours ago");
  assert.equal(relativeTime(now - DAY, now), "1 day ago");
  assert.equal(relativeTime(now - 2 * DAY, now), "2 days ago");
  assert.equal(relativeTime(now - 29 * DAY, now), "29 days ago");
  assert.equal(relativeTime(now - 30 * DAY, now), "1 month ago");
  assert.equal(relativeTime(now - 200 * DAY, now), "6 months ago");
  assert.equal(relativeTime(now - 365 * DAY, now), "1 year ago");
  assert.equal(relativeTime(now - 800 * DAY, now), "2 years ago");
});
