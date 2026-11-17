// What a first-time user is shown: the introduction's cards, a drawing and a few words each. Whether they've
// seen it is `{ introductionSeen }`, kept by the app (see `onboarding` in api.js).

/**
 * The introduction's cards, each `{ drawing, text }`, `drawing` naming its picture in drawings.js. The
 * last offers the Grouping Models, `modelsSize` (already formatted) in all, unless `modelsInstalled`.
 */
export function introductionCards(modelsInstalled, modelsSize) {
  const cards = [
    { drawing: "pick", text: "Pick a messy folder" },
    { drawing: "plan", text: "Check the Plan" },
    { drawing: "trash", text: "Everything goes to Trash. Undo anytime." },
  ];
  if (!modelsInstalled) {
    cards.push({ drawing: "download", text: `Read inside files? ${modelsSize}` });
  }
  return cards;
}
