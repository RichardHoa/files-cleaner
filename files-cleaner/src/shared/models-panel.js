// Follows the status event, so it shows a download started anywhere, even before this screen.
import {
  describeError,
  downloadGroupingModels,
  groupingModelsStatus,
  onGroupingModelsStatus,
  pathSeparator,
  pauseGroupingModels,
  removeGroupingModels,
} from "./api.js";
import { el, progressBar, showStatus } from "./dom.js";
import { formatSize } from "./format.js";
import { warningBlock } from "./outcomes.js";

export function thisComputer() {
  if (pathSeparator === "\\") return "PC";
  return /Mac/.test(navigator.userAgent) ? "Mac" : "computer";
}

export function groupingModelsPitch(total) {
  const computer = thisComputer();
  return `Read inside files for better folders · ${formatSize(total)} · stays on this ${computer}`;
}

// Screens are swapped without notice, so panels that left the page are dropped at the next status,
// and one listener serves them all.
const panels = new Set();
let listening = false;

function dropDetached() {
  for (const panel of panels) if (!panel.node.isConnected) panels.delete(panel);
}

function listen() {
  if (listening) return;
  listening = true;
  onGroupingModelsStatus((status) => {
    dropDetached();
    for (const panel of panels) panel.show(status);
  });
}

export function watchGroupingModels(node, error, show) {
  const panel = {
    node,
    show(status) {
      if (show(status) === false) panels.delete(panel);
    },
  };
  dropDetached();
  panels.add(panel);
  listen();
  groupingModelsStatus().then(
    (status) => panels.has(panel) && panel.show(status),
    (e) => showStatus(error, describeError(e), true),
  );
}

export function groupingModelsPanel(onInstalled) {
  const node = el("section", { className: "models-panel" });
  const error = el("p", { className: "status error" });
  const bar = progressBar();
  watchGroupingModels(node, error, (status) => {
    if (status.state === "installed") {
      onInstalled();
      return false;
    }
    node.replaceChildren(...groupingModelsControls(status, error, bar), error);
    showStatus(error, "");
  });
  node.append(error);
  return node;
}

/** The caller keeps `bar` across statuses, so it glides from one to the next. */
export function groupingModelsControls({ state, downloaded, total, error: failed }, error, bar) {
  const button = (label, command, primary = false) => modelsButton(label, command, error, primary);
  const say = (text) => (failed ? warningBlock(failed) : el("p", {}, text));
  const progress = () => {
    bar.set(downloaded, total);
    return bar.node;
  };
  const soFar = `${formatSize(downloaded)} of ${formatSize(total)}`;

  if (state === "downloading") {
    return [
      say(`Downloading the grouping models… ${soFar}`),
      progress(),
      el("div", { className: "actions" }, button("Pause", pauseGroupingModels), button("Cancel", removeGroupingModels)),
    ];
  }
  if (state === "paused") {
    return [
      say(`Download paused at ${soFar}.`),
      progress(),
      el(
        "div",
        { className: "actions" },
        button("Resume", downloadGroupingModels, true),
        button("Cancel", removeGroupingModels),
      ),
    ];
  }
  // The introduction tells the full story, so here it is only a tooltip.
  const pitch = say(groupingModelsPitch(total));
  if (!failed) pitch.title = groupingModelsDetails(formatSize(total), thisComputer()).join("\n\n");
  return [pitch, el("div", { className: "actions" }, button("Download", downloadGroupingModels, true))];
}

/** Disabled until the next status redraws it, so it can't be clicked twice. */
export function modelsButton(label, command, error, primary = false) {
  const node = el("button", { type: "button", className: primary ? "primary" : "" }, label);
  node.addEventListener("click", async () => {
    node.disabled = true;
    showStatus(error, "");
    try {
      await command();
    } catch (e) {
      showStatus(error, describeError(e), true);
      node.disabled = false;
    }
  });
  return node;
}

// Shared by the introduction's download card and the tooltips of shorter offers.

export function groupingModelsDetails(size, computer) {
  return [
    "To sort files by topic, the app reads inside them (text, PDFs, scanned pages) and works out what each one is about.",
    "That needs two open-source language models from Qwen: Qwen2.5-1.5B writes a short description of each file, and Qwen3-Embedding-0.6B compares the descriptions to find files that belong together.",
    `${size}, downloaded once from Hugging Face. They run on this ${computer}, so your files never leave it.`,
    "Without them, folders come from file kind, name and date. You can remove them anytime in Settings.",
  ];
}
