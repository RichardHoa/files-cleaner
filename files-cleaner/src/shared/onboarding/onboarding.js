import { groupingModelsDetails } from "../grouping-models/details.js";

/** Only the download card has `details`, as it asks for something. */
export function introductionCards(modelsInstalled, modelsSize, computer = "computer") {
  const cards = [
    { drawing: "pick", text: "Pick a messy folder" },
    { drawing: "plan", text: "Check the Plan" },
    { drawing: "trash", text: "Everything goes to Trash. Undo anytime." },
    { drawing: "rules", text: "Make Rules to sort files your way" },
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
