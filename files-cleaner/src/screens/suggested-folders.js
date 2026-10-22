// Suggested Folders: the step after the duplicate step, given a fresh scan of the Scan Root.
// It needs the Grouping Models: without them it offers them, or shows their download. With them,
// it is a placeholder with a way back to Home until the tool can suggest folders.
import { describeError, groupingModelsStatus } from "../api.js";
import { el } from "../dom.js";
import { formatSize, plural } from "../format.js";
import { icon } from "../icons.js";
import { groupingModelsPanel } from "../models-panel.js";

export async function render(root, nav, result) {
  const home = el("button", { type: "button", className: "primary" }, "Home");
  home.addEventListener("click", () => nav.home());

  const totalSize = result.entries.reduce((sum, entry) => sum + entry.size, 0);
  const body = el("div");
  root.append(
    el("h1", {}, "Suggested Folders"),
    el("p", { className: "path root-path" }, icon("folder"), result.root),
    el("p", { className: "lead" }, `Scanned again: ${plural(result.entries.length, "item")} · ${formatSize(totalSize)}`),
    body,
    el("div", { className: "actions" }, home),
  );

  const withModels = () =>
    body.replaceChildren(el("p", {}, "Suggested Folders are coming. Soon the tool will suggest folders to gather what's left into."));
  const withoutModels = () =>
    body.replaceChildren(
      groupingModelsPanel(withModels),
      el("p", { className: "note" }, "Sorting by kind, name and date, without the grouping models, is coming."),
    );

  try {
    const { state } = await groupingModelsStatus();
    if (state === "installed") withModels();
    else withoutModels();
  } catch (error) {
    body.replaceChildren(el("p", { className: "status error" }, describeError(error)));
  }
}
