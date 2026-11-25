// Declining the models here isn't final: the Suggested Folders step still offers them.
import { describeError, downloadGroupingModels, groupingModelsStatus, onboarding, saveOnboarding } from "./api.js";
import { el, showStatus } from "./dom.js";
import { formatSize } from "./format.js";
import { groupingModelsDetails, thisComputer } from "./models-panel.js";

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

// Loaded once, saved on every change.

let state = null;

/** If it can't be loaded it counts as seen, so nothing nags. */
export function seen() {
  state ??= onboarding().catch(() => ({ introductionSeen: true, multiSelectSeen: true }));
  return state;
}

export async function updateSeen(change) {
  const next = change(await seen());
  state = Promise.resolve(next);
  try {
    await saveOnboarding(next);
  } catch {
    // Only the memory of it is lost: it shows again next time.
  }
}

// 160×100 grid, stroked in the accent colour (see `.intro-drawing`).
const SVG = "http://www.w3.org/2000/svg";

const DOC = "M0 0h14l6 6v20H0z M14 0v6h6";

const DRAWINGS = {
  pick: [
    ["M50 50h20l6 6h34a4 4 0 0 1 4 4v26a4 4 0 0 1-4 4H50a4 4 0 0 1-4-4V54a4 4 0 0 1 4-4z"],
    [DOC, "translate(40 14) rotate(-12)"],
    [DOC, "translate(70 6) rotate(4)"],
    [DOC, "translate(102 12) rotate(14)"],
  ],
  plan: [
    ["M28 8h104a4 4 0 0 1 4 4v76a4 4 0 0 1-4 4H28a4 4 0 0 1-4-4V12a4 4 0 0 1 4-4z"],
    ["M40 18h12v12H40z M43 24l3 3 5-6 M62 24h58"],
    ["M40 44h12v12H40z M43 50l3 3 5-6 M62 50h44"],
    ["M40 70h12v12H40z M62 76h52"],
  ],
  trash: [
    ["M76 30h40 M90 30v-5h12v5 M80 30l3 52h26l3-52 M91 40v32 M101 40v32"],
    ["M70 60H52a12 12 0 0 1 0-24h14 M61 31l5 5-5 5"],
  ],
  rules: [
    [DOC, "translate(14 40) rotate(-10)"],
    [DOC, "translate(34 32) rotate(6)"],
    ["M62 62h18 M75 57l5 5-5 5"],
    ["M92 46h20l6 6h28a4 4 0 0 1 4 4v28a4 4 0 0 1-4 4H92a4 4 0 0 1-4-4V50a4 4 0 0 1 4-4z"],
    ["M101 64h38v12h-38z M107 70h26"],
  ],
  download: [
    ["M40 18h22l10 10v50H40z M62 18v10h10 M48 40h16 M48 50h16 M48 60h10"],
    ["M106 22v36 M94 46l12 12 12-12 M88 66v12h36V66"],
  ],
};

/** Decorative: the sentence beside it says what it shows. */
export function drawing(name) {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 160 100");
  svg.setAttribute("class", "intro-drawing");
  svg.setAttribute("aria-hidden", "true");
  for (const [d, transform] of DRAWINGS[name]) {
    const path = document.createElementNS(SVG, "path");
    path.setAttribute("d", d);
    if (transform) path.setAttribute("transform", transform);
    svg.append(path);
  }
  return svg;
}
