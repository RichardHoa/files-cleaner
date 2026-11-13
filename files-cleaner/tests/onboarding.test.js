import assert from "node:assert/strict";
import { test } from "node:test";
import { guideShown, introductionCards, withGuideDismissed } from "../src/shared/onboarding/onboarding.js";

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

test("a dismissed guide stays hidden, and the others still show", () => {
  const state = withGuideDismissed({ introductionSeen: true, dismissedGuides: [] }, "review");

  assert.equal(guideShown(state, "review"), false);
  assert.equal(guideShown(state, "home"), true);
  assert.deepEqual(withGuideDismissed(state, "review").dismissedGuides, ["review"], "dismissing twice keeps one entry");
});
