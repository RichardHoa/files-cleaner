// The introduction: a sheet of cards, one at a time, shown once over Home and again from Settings. Its
// last card offers the Grouping Models; after "Not now" they're still offered in the Suggested Folders step.
import { describeError, downloadGroupingModels, groupingModelsStatus } from "../api.js";
import { el } from "../dom.js";
import { formatSize } from "../format.js";
import { showStatus } from "../status.js";
import { drawing } from "./drawings.js";
import { introductionCards } from "./onboarding.js";
import { seen, updateSeen } from "./seen.js";

/** Shows the introduction unless the user has finished or skipped it before. */
export async function showIntroductionOnce() {
  if (!(await seen()).introductionSeen) showIntroduction();
}

/** Shows the introduction; closing it, however, counts as having seen it. */
export async function showIntroduction() {
  let models = null;
  try {
    models = await groupingModelsStatus();
  } catch {
    // The download card is left out: the Suggested Folders step still offers the models.
  }
  // A download already started counts as installed here: the Suggested Folders step shows its progress.
  const cards = introductionCards(models?.state !== "notInstalled", models && formatSize(models.total));

  let index = 0;
  const picture = el("div", { className: "intro-picture" });
  const text = el("p", { className: "intro-text" });
  const dots = el("div", { className: "intro-dots", ariaHidden: "true" }, ...cards.map(() => el("span")));
  const error = el("p", { className: "status error" });
  const buttons = el("div", { className: "actions" });
  const sheet = el("dialog", { className: "sheet intro", ariaLabel: "Introduction" }, picture, text, dots, error, buttons);

  const close = () => {
    updateSeen((state) => ({ ...state, introductionSeen: true }));
    sheet.close();
    sheet.remove();
  };
  const button = (label, onClick, className = "") => {
    const node = el("button", { type: "button", className }, label);
    node.addEventListener("click", onClick);
    return node;
  };
  const go = (step) => () => {
    index += step;
    draw();
  };

  const download = async (event) => {
    const node = event.currentTarget;
    node.disabled = true;
    showStatus(error, "");
    try {
      await downloadGroupingModels();
      close();
    } catch (e) {
      // Too little space, say: the user reads why and can still say "Not now".
      showStatus(error, describeError(e), true);
      node.disabled = false;
    }
  };

  function draw() {
    const card = cards[index];
    picture.replaceChildren(drawing(card.drawing));
    text.textContent = card.text;
    dots.childNodes.forEach((dot, i) => dot.classList.toggle("current", i === index));
    showStatus(error, "");
    const back = index > 0 ? [button("Back", go(-1))] : [];
    let primary;
    if (card.drawing === "download") {
      primary = button("Download", download, "primary");
      buttons.replaceChildren(...back, button("Not now", close), primary);
    } else if (index === cards.length - 1) {
      primary = button("Done", close, "primary");
      buttons.replaceChildren(...back, primary);
    } else {
      primary = button("Next", go(1), "primary");
      buttons.replaceChildren(button("Skip", close, "link intro-skip"), ...back, primary);
    }
    primary.focus();
  }

  // Escape skips, as "Not now" on the download card.
  sheet.addEventListener("cancel", (event) => {
    event.preventDefault();
    close();
  });

  document.body.append(sheet);
  sheet.showModal();
  draw();
}
