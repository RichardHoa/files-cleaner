import assert from "node:assert/strict";
import { test } from "node:test";
import { fitTail } from "../src/format.js";

const upTo = (n) => (text) => text.length <= n;

test("a path that fits is left whole", () => {
  assert.equal(fitTail("a/b/c.txt", "/", upTo(9)), "a/b/c.txt");
});

test("a long path loses whole folders from its start", () => {
  const path = "linux/Development/flutter/bin/cache/flutter.jar";
  assert.equal(fitTail(path, "/", upTo(30)), "…/bin/cache/flutter.jar");
  assert.equal(fitTail(path, "/", upTo(25)), "…/bin/cache/flutter.jar");
  assert.equal(fitTail(path, "/", upTo(23)), "…/bin/cache/flutter.jar");
  assert.equal(fitTail(path, "/", upTo(22)), "…/cache/flutter.jar");
});

test("keeps the longest tail that fits", () => {
  const path = "a/bb/ccc/dddd/e";
  for (let n = 3; n < path.length; n += 1) {
    const fitted = fitTail(path, "/", upTo(n));
    assert.ok(fitted.length <= n, `${fitted} fits in ${n}`);
    assert.ok(fitted.startsWith("…/"));
    assert.ok(path.endsWith(fitted.slice(1)));
  }
  assert.equal(fitTail(path, "/", upTo(9)), "…/dddd/e");
});

test("a file name too long on its own is cut character by character", () => {
  assert.equal(fitTail("docs/abcdefghij.pdf", "/", upTo(8)), "…hij.pdf");
  assert.equal(fitTail("abcdefghij.pdf", "/", upTo(8)), "…hij.pdf");
});

test("when nothing fits, the last character is still shown", () => {
  assert.equal(fitTail("docs/abc", "/", upTo(0)), "…c");
});

test("folders are dropped at the given separator", () => {
  assert.equal(fitTail("C:\\Users\\me\\notes.txt", "\\", upTo(11)), "…\\notes.txt");
});
