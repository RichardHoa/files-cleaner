import assert from "node:assert/strict";
import { test } from "node:test";
import { splitPath } from "../src/format.js";

test("a folder's name and the path of the folder holding it", () => {
  assert.deepEqual(splitPath("/Users/me/Downloads", "/"), { name: "Downloads", parent: "/Users/me" });
  assert.deepEqual(splitPath("/Users", "/"), { name: "Users", parent: "/" });
  assert.deepEqual(splitPath("C:\\Users\\me", "\\"), { name: "me", parent: "C:\\Users" });
});

test("a trailing separator is ignored", () => {
  assert.deepEqual(splitPath("/Users/me/", "/"), { name: "me", parent: "/Users" });
});

test("a disk root is its own name, with no parent", () => {
  assert.deepEqual(splitPath("/", "/"), { name: "/", parent: "" });
  assert.deepEqual(splitPath("C:\\", "\\"), { name: "C:\\", parent: "" });
});
