// The Grouping Models' offer, or their download's progress with Pause, Resume and Cancel. Kept up to
// date from the status event, so it shows a download started anywhere, even before this screen.
import {
  describeError,
  downloadGroupingModels,
  groupingModelsStatus,
  onGroupingModelsStatus,
  pathSeparator,
  pauseGroupingModels,
  removeGroupingModels,
} from "./api.js";
import { el } from "./dom.js";
import { formatSize } from "./format.js";
import { showStatus } from "./status.js";

/** "Mac", "PC" or "computer": what the user calls this machine, for "nothing leaves your Mac". */
function thisComputer() {
  if (pathSeparator === "\\") return "PC";
  return /Mac/.test(navigator.userAgent) ? "Mac" : "computer";
}

/** What the Grouping Models are, `total` bytes in all, and that nothing leaves the computer: for the offer and the panel. */
export function groupingModelsPitch(total) {
  const computer = thisComputer();
  return (
    `Files Cleaner can download grouping models (${formatSize(total)}) to read your files on this ${computer} ` +
    `and suggest folders for them. Nothing leaves your ${computer}.`
  );
}

// The panels on screen, each a function taking the latest status, fed by one listener for the whole
// app. Screens are swapped without notice, so a panel that has left the page is dropped at the next
// status or the next new panel, whichever comes first.
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

/** The panel; calls `onInstalled()` once the models are installed, and stops updating then. */
export function groupingModelsPanel(onInstalled) {
  const node = el("section", { className: "models-panel" });
  const error = el("p", { className: "status error" });
  const panel = {
    node,
    show(status) {
      if (status.state === "installed") {
        panels.delete(panel);
        onInstalled();
        return;
      }
      node.replaceChildren(...describe(status, error), error);
      showStatus(error, status.error ?? "", status.error != null);
    },
  };

  dropDetached();
  panels.add(panel);
  listen();
  groupingModelsStatus().then(
    (status) => panels.has(panel) && panel.show(status),
    (e) => showStatus(error, describeError(e), true),
  );
  node.append(error);
  return node;
}

/** The panel's text, progress and buttons for `status`; a failing button says why in `error`. */
function describe({ state, downloaded, total }, error) {
  const button = (label, command, primary = false) => {
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
  };
  const progress = () => el("progress", { max: total, value: downloaded });
  const soFar = `${formatSize(downloaded)} of ${formatSize(total)}`;

  if (state === "downloading") {
    return [
      el("p", {}, `Downloading the grouping models… ${soFar}`),
      progress(),
      el("div", { className: "actions" }, button("Pause", pauseGroupingModels), button("Cancel", removeGroupingModels)),
    ];
  }
  if (state === "paused") {
    return [
      el("p", {}, `Download paused at ${soFar}.`),
      progress(),
      el(
        "div",
        { className: "actions" },
        button("Resume", downloadGroupingModels, true),
        button("Cancel", removeGroupingModels),
      ),
    ];
  }
  return [
    el("p", {}, groupingModelsPitch(total)),
    el("div", { className: "actions" }, button("Download", downloadGroupingModels, true)),
  ];
}
