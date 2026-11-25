import assert from "node:assert/strict";
import { test } from "node:test";
import { introductionCards } from "../src/shared/introduction.js";

test("the introduction shows Rules before offering the Grouping Models last, and leaves that card out once they're installed", () => {
  assert.deepEqual(
    introductionCards(false, "1.8 GB").map((card) => card.drawing),
    ["pick", "plan", "trash", "rules", "download"],
  );
  assert.deepEqual(
    introductionCards(true, "1.8 GB").map((card) => card.drawing),
    ["pick", "plan", "trash", "rules"],
  );
});

test("each introduction card's caption is a few words, the drawing saying the rest", () => {
  // Only the download card may say more.
  for (const card of introductionCards(false, "1.8 GB").filter((card) => card.drawing !== "download")) {
    assert.ok(card.text.split(/\s+/).length <= 7, card.text);
  }
});

test("the download card says which models are downloaded, how big they are and what they're for", () => {
  const card = introductionCards(false, "1.8 GB", "Mac").at(-1);
  const details = card.details.join("\n");
  for (const fact of ["Qwen2.5-1.5B", "Qwen3-Embedding-0.6B", "1.8 GB", "Hugging Face", "this Mac", "Settings"]) {
    assert.ok(details.includes(fact), fact);
  }
});
