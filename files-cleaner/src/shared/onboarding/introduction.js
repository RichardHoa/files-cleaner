// Declining the models here isn't final: the Suggested Folders step still offers them.
import { describeError, downloadGroupingModels, groupingModelsStatus } from "../api.js";
import { el } from "../dom.js";
import { formatSize } from "../format.js";
import { thisComputer } from "../grouping-models/models-panel.js";
import { showStatus } from "../status.js";
import { drawing } from "./drawings.js";
import { introductionCards } from "./onboarding.js";
import { seen, updateSeen } from "./seen.js";

export async function showIntroductionOnce() {
  if (!(await seen()).introductionSeen) showIntroduction();
}

/** Closing it in any way counts as having seen it. */
export async function showIntroduction() {
  let models = null;
  try {
    models = await groupingModelsStatus();
  } catch {
    // The Suggested Folders step still offers the models.
  }
  // A download already started counts too: the Suggested Folders step shows its progress.
  const cards = introductionCards(models?.state !== "notInstalled", models && formatSize(models.total), thisComputer());

  let index = 0;
  const picture = el("div", { className: "intro-picture" });
  const text = el("p", { className: "intro-text" });
  // Only the download card asks for something, so only it explains more.
  const details = el("div", { className: "intro-details" });
  const dots = el("div", { className: "intro-dots", ariaHidden: "true" }, ...cards.map(() => el("span")));
  const error = el("p", { className: "status error" });
  const buttons = el("div", { className: "actions" });
  const sheet = el("dialog", { className: "sheet intro", ariaLabel: "Introduction" }, picture, text, details, dots, error, buttons);

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
      // Too little space, say: the user can still choose "Not now".
      showStatus(error, describeError(e), true);
      node.disabled = false;
    }
  };

  function draw() {
    const card = cards[index];
    picture.replaceChildren(drawing(card.drawing));
    text.textContent = card.text;
    details.replaceChildren(...(card.details ?? []).map((paragraph) => el("p", {}, paragraph)));
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

  // Escape skips, as "Not now" does.
  sheet.addEventListener("cancel", (event) => {
    event.preventDefault();
    close();
  });

  document.body.append(sheet);
  sheet.showModal();
  draw();
}
