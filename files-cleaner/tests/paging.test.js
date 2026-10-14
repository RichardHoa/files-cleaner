import assert from "node:assert/strict";
import { test } from "node:test";
import { pageCount, pageNumbers } from "../src/paging.js";

// `null` is a gap, shown as "…".

test("no Sessions means no pages", () => {
  assert.equal(pageCount(0, 10), 0);
  assert.deepEqual(pageNumbers(1, 0), []);
});

test("a single page", () => {
  assert.equal(pageCount(7, 10), 1);
  assert.equal(pageCount(10, 10), 1);
  assert.deepEqual(pageNumbers(1, 1), [1]);
});

test("a partial last page still counts", () => {
  assert.equal(pageCount(11, 10), 2);
});

test("a few pages are all shown", () => {
  assert.deepEqual(pageNumbers(3, 5), [1, 2, 3, 4, 5]);
});

test("on the first page: its neighbour, then a gap to the last", () => {
  assert.deepEqual(pageNumbers(1, 10), [1, 2, null, 10]);
});

test("in the middle: the first, the neighbours either side, and the last", () => {
  assert.deepEqual(pageNumbers(5, 10), [1, null, 4, 5, 6, null, 10]);
});

test("on the last page: the first, a gap, then its neighbour", () => {
  assert.deepEqual(pageNumbers(10, 10), [1, null, 9, 10]);
});

test("a gap of one page shows that page instead", () => {
  assert.deepEqual(pageNumbers(4, 10), [1, 2, 3, 4, 5, null, 10]);
  assert.deepEqual(pageNumbers(7, 10), [1, null, 6, 7, 8, 9, 10]);
});
