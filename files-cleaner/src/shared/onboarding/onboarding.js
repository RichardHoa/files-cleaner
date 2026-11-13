// What a first-time user is told: the introduction's cards, and a guide card per screen. The state saying
// what they've seen is `{ introductionSeen, dismissedGuides }`, kept by the app (see `onboarding` in api.js).

/**
 * The introduction's cards, each `{ drawing, text }`, `drawing` naming its picture in drawings.js. The
 * last offers the Grouping Models, `modelsSize` (already formatted) in all, unless `modelsInstalled`.
 */
export function introductionCards(modelsInstalled, modelsSize) {
  const cards = [
    { drawing: "pick", text: "Pick a messy folder, such as Downloads." },
    { drawing: "plan", text: "It finds duplicates and suggests folders, and you see the full Plan before anything changes." },
    { drawing: "trash", text: "Nothing is deleted: everything goes to the System Trash, and you can undo it all." },
  ];
  if (!modelsInstalled) {
    cards.push({ drawing: "download", text: `Read inside files for better folders? One-time ${modelsSize} download.` });
  }
  return cards;
}

/** The guide card of each screen but Settings: its title and what it says, a line each. */
export const GUIDES = {
  home: {
    title: "Start here",
    lines: [
      "Pick a folder to clean, or click one you scanned before to scan it again.",
      "Folders the system and your apps need, and ones you list in Settings, are left alone.",
      "Nothing changes until you approve a Plan.",
    ],
  },
  review: {
    title: "Check the Plan",
    lines: [
      "Duplicates lists files with identical copies; the ticked copies go to the Trash.",
      "Suggested Folders gathers loose files into folders. Untick anything you'd rather leave as it is.",
      "Nothing happens until you click the button at the top.",
    ],
  },
  results: {
    title: "What happened",
    lines: [
      "Each card shows what was done, and what was skipped or failed, with the reason beside each file.",
      "Put back undoes one file or all of them.",
    ],
  },
  history: {
    title: "Past Sessions",
    lines: [
      "Each time you run a Plan, it becomes a Session. Open one to see its files and put them back.",
      "Files emptied from the System Trash can't be put back.",
    ],
  },
};

/** Whether the guide card `name` shows on its own in `state`: until it's dismissed. */
export const guideShown = (state, name) => !state.dismissedGuides.includes(name);

/** `state` with the guide card `name` dismissed. */
export function withGuideDismissed(state, name) {
  if (!guideShown(state, name)) return state;
  return { ...state, dismissedGuides: [...state.dismissedGuides, name] };
}
