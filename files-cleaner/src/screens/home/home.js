import {
  cancelScan,
  describeError,
  listSessions,
  loadConfig,
  pathSeparator,
  pickFolder,
  recentScanRoots,
  removeRecentScanRoot,
  scanRootWarning,
} from "../../shared/api.js";
import { el, icon, showStatus, showWarning } from "../../shared/dom.js";
import { count, relativeTime, splitPath } from "../../shared/format.js";
import { refreshMenu, setScreenMenu, withShortcut } from "../../shared/menus.js";
import { undoSections } from "../../shared/outcomes.js";
import { noSessions, sessionList } from "../history/history.js";

const LATEST_SESSIONS = 3;

export function render(root, nav, start = null) {
  const badge = el("button", { type: "button", className: "config-badge" });
  const rulesButton = el("button", { type: "button", className: "config-badge", title: "How files are sorted at every scan" }, "Rules");
  rulesButton.addEventListener("click", () => nav.rules("home"));
  const configError = el("p", { className: "status error" });
  const tiles = el("div", { className: "tiles" });
  const status = el("p", { className: "status" });
  const cancelButton = el("button", { type: "button", className: "small" }, "Cancel");
  const scanActions = el("p", { hidden: true }, cancelButton);
  const sessions = el("div");
  const seeAll = el("button", { type: "button", className: "link", hidden: true });
  seeAll.addEventListener("click", () => nav.history(1));

  root.append(
    el("header", { className: "home-header" }, el("h1", {}, "Files Cleaner"), el("div", { className: "home-header-actions" }, rulesButton, badge)),
    configError,
    el("p", { className: "lead" }, "Pick a folder to clean"),
    tiles,
    status,
    scanActions,
    el("div", { className: "section-head" }, el("h2", {}, "Sessions"), seeAll),
    sessions,
  );

  let scanning = false;
  setScreenMenu(() => (scanning ? { newScan: null, recent: [] } : {}));
  // Home stays as it was, with nothing kept.
  let cancelled = false;
  const cancel = () => {
    if (cancelled) return;
    cancelled = true;
    cancelButton.disabled = true;
    showStatus(status, "Stopping…");
    cancelScan().catch((error) => showWarning(status, describeError(error)));
  };
  cancelButton.addEventListener("click", cancel);
  const cancelOnEscape = (event) => {
    // Home was left while scanning: Escape belongs to the screen shown now.
    if (!cancelButton.isConnected) return document.removeEventListener("keydown", cancelOnEscape);
    if (event.key === "Escape") cancel();
  };

  const startScan = async (folder) => {
    if (scanning) return;
    scanning = true;
    refreshMenu();
    tiles.classList.add("busy");
    try {
      const warning = await scanRootWarning(folder);
      if (warning && !(await confirmScan(status, warning))) {
        status.replaceChildren();
      } else {
        cancelled = false;
        cancelButton.disabled = false;
        scanActions.hidden = false;
        document.addEventListener("keydown", cancelOnEscape);
        showStatus(status, `Scanning ${folder}…`);
        try {
          await nav.steps.scanFolder(folder, (line) => cancelled || showStatus(status, line), () => cancelled);
          if (!cancelled) return;
          status.replaceChildren();
        } finally {
          scanActions.hidden = true;
          document.removeEventListener("keydown", cancelOnEscape);
        }
      }
    } catch (error) {
      // The scan or the search for duplicates failed as a whole, or was cancelled.
      if (cancelled) status.replaceChildren();
      else showWarning(status, describeError(error));
    }
    scanning = false;
    refreshMenu();
    tiles.classList.remove("busy");
  };

  const pick = async () => {
    const folder = await pickFolder();
    if (folder) startScan(folder);
  };

  badge.addEventListener("click", () => nav.settings());

  showConfig(badge, configError);
  showTiles(tiles, startScan, pick);
  showSessions(sessions, seeAll, nav);

  if (start?.pick) pick();
  else if (start?.folder) startScan(start.folder);
}

async function showConfig(badge, errorLine) {
  try {
    await loadConfig();
    badge.classList.remove("error");
    badge.title = withShortcut("Open Settings", "settings");
    badge.replaceChildren(icon("settings"), "Settings");
    errorLine.textContent = "";
  } catch (error) {
    badge.classList.add("error");
    badge.title = withShortcut("Open Settings", "settings");
    badge.replaceChildren(icon("settings"), "Settings: config error");
    errorLine.textContent = describeError(error);
  }
}

async function showTiles(node, startScan, pick) {
  const pickTile = el(
    "button",
    {
      type: "button",
      className: "tile pick-tile",
      title: "Duplicates are found anywhere inside. Only files directly in the folder are sorted into folders.",
    },
    icon("plus", "tile-icon"),
    el("span", { className: "tile-name" }, "Pick folder"),
    el("span", { className: "tile-detail" }, "Choose any folder to clean"),
  );
  pickTile.addEventListener("click", pick);

  let recent = [];
  try {
    recent = await recentScanRoots();
  } catch {
    // Only the shortcut is lost; picking still works.
  }
  const redraw = () => showTiles(node, startScan, pick);
  node.replaceChildren(...recent.map((folder) => recentTile(folder, startScan, redraw)), pickTile);
}

function recentTile({ path, lastScanned, exists }, startScan, redraw) {
  const { name, parent } = splitPath(path, pathSeparator);
  const remove = el("button", { type: "button", className: "tile-remove", title: "Remove from this list" }, icon("close"));
  remove.ariaLabel = `Remove ${name} from this list`;
  remove.addEventListener("click", async (event) => {
    event.stopPropagation();
    try {
      await removeRecentScanRoot(path);
    } finally {
      redraw();
    }
  });
  const tile = el(
    "div",
    { className: exists ? "tile" : "tile missing", title: exists ? `Scan ${path}` : `Not found: ${path}` },
    icon("folder", "tile-icon"),
    el("span", { className: "tile-name" }, name),
    el("span", { className: "tile-detail path" }, parent),
    el("span", { className: "tile-detail" }, exists ? relativeTime(lastScanned) : "Folder not found"),
    remove,
  );
  // Not a <button>: it holds the remove button, and buttons can't nest.
  tile.role = "button";
  tile.ariaDisabled = String(!exists);
  tile.tabIndex = exists ? 0 : -1;
  if (exists) {
    tile.addEventListener("click", () => startScan(path));
    tile.addEventListener("keydown", (event) => {
      if ((event.key === "Enter" || event.key === " ") && event.target === tile) {
        event.preventDefault();
        startScan(path);
      }
    });
  }
  return tile;
}

async function showSessions(node, seeAll, nav, undoResult = null) {
  let page;
  try {
    page = await listSessions(0, LATEST_SESSIONS);
  } catch (error) {
    node.replaceChildren(el("p", { className: "status error" }, describeError(error)));
    return;
  }
  seeAll.hidden = page.total === 0;
  seeAll.textContent = `See all (${count(page.total)})`;
  const list =
    page.sessions.length > 0
      ? sessionList(page.sessions, {
          open: (id) => nav.session(id),
          undone: (result) => showSessions(node, seeAll, nav, result),
        })
      : noSessions();
  node.replaceChildren(...(undoResult ? undoSections(undoResult, pathSeparator) : []), list);
}

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
