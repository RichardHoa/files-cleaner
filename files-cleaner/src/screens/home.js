// Home: pick a Scan Root, open the config, see whether the config is valid.
import { describeError, loadConfig, openConfig, pickFolder, scan } from "../api.js";
import { el } from "../dom.js";
import { plural } from "../format.js";

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
    showMessage(status, `Scanning ${folder}…`);
    try {
      nav.scanResults(await scan(folder));
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

function showMessage(node, text, isError = false) {
  node.textContent = text;
  node.classList.toggle("error", isError);
}

