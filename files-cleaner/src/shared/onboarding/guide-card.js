// A screen's guide card, shown at its top the first time it's reached, and the "?" in its action bar that
// brings it back.
import { el } from "../dom.js";
import { GUIDES, guideShown, withGuideDismissed } from "./onboarding.js";
import { seen, updateSeen } from "./seen.js";

/**
 * The guide card `name` (from onboarding.js) as `{ card, help }`: the card, shown once it's known not to be
 * dismissed, and the "?" button showing it again. "Got it" hides it for good.
 */
export function guide(name) {
  const { title, lines } = GUIDES[name];
  const gotIt = el("button", { type: "button" }, "Got it");
  const card = el(
    "section",
    { className: "guide-card", hidden: true },
    el("h2", {}, title),
    ...lines.map((line) => el("p", {}, line)),
    el("div", { className: "actions" }, gotIt),
  );
  const help = el(
    "button",
    { type: "button", className: "help-button", title: "What's on this screen", ariaLabel: "Show the guide for this screen" },
    "?",
  );

  gotIt.addEventListener("click", () => {
    card.hidden = true;
    updateSeen((state) => withGuideDismissed(state, name));
  });
  help.addEventListener("click", () => {
    card.hidden = false;
    card.scrollIntoView({ block: "nearest" });
  });
  seen().then((state) => {
    if (guideShown(state, name)) card.hidden = false;
  });
  return { card, help };
}

/** Shows every guide card again, the next time its screen is reached. */
export const showAllGuidesAgain = () => updateSeen((state) => ({ ...state, dismissedGuides: [] }));
