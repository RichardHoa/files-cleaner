import assert from "node:assert/strict";
import { test } from "node:test";
import { describeRule, parseExtensions, parseMeaningFolders, parseWords } from "../src/screens/rules/rule-text.js";

test("a Rule reads back in one line", () => {
  const rule = {
    enabled: true,
    match: { extensions: ["png"], name: { how: "pattern", words: ["Screenshot*"] }, date: { field: "changed", withinDays: 30 } },
    groupBy: { type: "folder", name: "Screenshots/{month}", date: "changed" },
  };
  assert.equal(describeRule(rule), ".png files whose name matches “Screenshot*” changed in the last 30 days → “Screenshots/{month}”");
  assert.equal(
    describeRule({ match: { name: { how: "contains", words: ["baraka", "trip"] }, exceptNames: ["draft"] }, groupBy: { type: "kind" } }),
    "Files whose name contains “baraka” or “trip” except “draft” → by kind",
  );
  assert.equal(describeRule({ match: {}, groupBy: { type: "date", date: "changed", by: "buckets" } }), "Files → Today, Yesterday, Older…");
});

test("extensions are read from a typed list", () => {
  assert.deepEqual(parseExtensions("png, .JPG  gif"), ["png", "jpg", "gif"]);
  assert.deepEqual(parseWords(" baraka, trip to ,"), ["baraka", "trip to"]);
});

test("a Rule by meaning's folders are read one a line, with what goes in each", () => {
  const folders = parseMeaningFolders("Thesis: chapters and drafts\n\n Taxes \n");
  assert.deepEqual(folders, [{ name: "Thesis", description: "chapters and drafts" }, { name: "Taxes" }]);
  assert.equal(describeRule({ match: {}, groupBy: { type: "meaning", folders } }), "Files → by meaning: “Thesis” or “Taxes”");
});
