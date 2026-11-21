// What a first-time user is shown: the introduction's cards, a drawing and a few words each. Whether they've
// seen it is `{ introductionSeen }`, kept by the app (see `onboarding` in api.js).
import { groupingModelsDetails } from "../grouping-models/details.js";

/**
 * The introduction's cards, each `{ drawing, text }`, `drawing` naming its picture in drawings.js. The
 * last offers the Grouping Models, `modelsSize` (already formatted) in all, unless `modelsInstalled`; it
 * alone has `details`, the paragraphs saying what is downloaded and why, `computer` being this machine.
 */
export function introductionCards(modelsInstalled, modelsSize, computer = "computer") {
  const cards = [
    { drawing: "pick", text: "Pick a messy folder" },
    { drawing: "plan", text: "Check the Plan" },
    { drawing: "trash", text: "Everything goes to Trash. Undo anytime." },
  ];
  if (!modelsInstalled) {
    cards.push({
      drawing: "download",
      text: "Suggest folders by what's inside files",
      details: groupingModelsDetails(modelsSize, computer),
    });
  }
  return cards;
}
