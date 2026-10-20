// Suggested Folders: the step after the duplicate step, given a fresh scan of the Scan Root.
// Until the tool can suggest folders, this is a placeholder with a way back to Home.
import { el } from "../dom.js";
import { formatSize, plural } from "../format.js";
import { icon } from "../icons.js";

export function render(root, nav, result) {
  const home = el("button", { type: "button", className: "primary" }, "Home");
  home.addEventListener("click", () => nav.home());

  const totalSize = result.entries.reduce((sum, entry) => sum + entry.size, 0);
  root.append(
    el("h1", {}, "Suggested Folders"),
    el("p", { className: "path root-path" }, icon("folder"), result.root),
    el("p", { className: "lead" }, `Scanned again: ${plural(result.entries.length, "item")} · ${formatSize(totalSize)}`),
    el("p", {}, "Suggested Folders are coming. Soon the tool will suggest folders to gather what's left into."),
    el("div", { className: "actions" }, home),
  );
}
