import assert from "node:assert/strict";
import { test } from "node:test";
import { introductionCards } from "../src/shared/onboarding/onboarding.js";

test("the introduction offers the Grouping Models last, and leaves that card out once they're installed", () => {
  assert.deepEqual(
    introductionCards(false, "1.8 GB").map((card) => card.drawing),
    ["pick", "plan", "trash", "download"],
  );
  assert.deepEqual(
    introductionCards(true, "1.8 GB").map((card) => card.drawing),
    ["pick", "plan", "trash"],
  );
});

test("each introduction card's caption is a few words, the drawing saying the rest", () => {
  for (const card of introductionCards(false, "1.8 GB")) {
    assert.ok(card.text.split(/\s+/).length <= 6, card.text);
  }
});
