// Home: pick a Scan Root, open the config, see whether the config is valid.
import { describeError, loadConfig, openConfig, pickFolder, scan, scanRootWarning } from "../api.js";
import { el } from "../dom.js";
import { count, plural } from "../format.js";

export function render(root, nav) {
  const pick = el("button", { type: "button", className: "primary" }, "Pick folder");
  const open = el("button", { type: "button" }, "Open config");
  const status = el("p", { className: "status" });
  const configStatus = el("p", { className: "status" });

  root.append(
    el("h1", {}, "Files Cleaner"),
    el("p", { className: "lead" }, "Pick a folder to scan. Nothing changes until you approve a Plan."),
    el("div", { className: "actions" }, pick, open),
    status,
    configStatus,
  );

  showConfigStatus(configStatus);

  pick.addEventListener("click", async () => {
    const folder = await pickFolder();
    if (!folder) return;
    pick.disabled = true;
    try {
      // A protected folder can still be scanned, once the user has read why it's protected.
      const warning = await scanRootWarning(folder);
      if (warning && !(await confirmScan(status, warning))) {
        status.replaceChildren();
        pick.disabled = false;
        return;
      }
      showMessage(status, `Scanning ${folder}…`);
      nav.scanResults(await scan(folder, (progress) => showProgress(status, folder, progress)));
    } catch (error) {
      showMessage(status, describeError(error), true);
      pick.disabled = false;
    }
  });

  open.addEventListener("click", async () => {
    try {
      await openConfig();
    } catch (error) {
      showMessage(status, describeError(error), true);
    }
  });

  // The config is re-read on every use, so refresh this when the user comes
  // back from editing it.
  const refresh = () => {
    if (configStatus.isConnected) showConfigStatus(configStatus);
    else window.removeEventListener("focus", refresh);
  };
  window.addEventListener("focus", refresh);
}

async function showConfigStatus(node) {
  try {
    const { path, config } = await loadConfig();
    showMessage(node, `Config: ${plural(config.rules.length, "Rule")} in ${path}`);
  } catch (error) {
    showMessage(node, `Config error: ${describeError(error)}`, true);
  }
}

function showProgress(node, folder, { found, folders, skipped, current }) {
  const counts = [`${plural(found, "file")} found`, `${plural(folders, "folder")} read`, `${count(skipped)} protected skipped`];
  showMessage(node, `Scanning ${folder}…\n${counts.join(" · ")}\n${current}`);
}

/** Shows `warning` in `node` with "Scan anyway" and "Cancel"; resolves to whether to scan. */
function confirmScan(node, warning) {
  return new Promise((resolve) => {
    const scanAnyway = el("button", { type: "button", className: "primary" }, "Scan anyway");
    const cancel = el("button", { type: "button" }, "Cancel");
    scanAnyway.addEventListener("click", () => resolve(true));
    cancel.addEventListener("click", () => resolve(false));
    node.classList.remove("error");
    node.replaceChildren(el("span", { className: "warning" }, `${warning}.`), el("div", { className: "actions" }, scanAnyway, cancel));
  });
}

function showMessage(node, text, isError = false) {
  node.textContent = text;
  node.classList.toggle("error", isError);
}

