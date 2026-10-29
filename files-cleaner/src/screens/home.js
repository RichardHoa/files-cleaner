// Home: rescan a Recent Scan Root or pick a folder, open Settings, and see the latest Sessions.
import {
  describeError,
  listSessions,
  loadConfig,
  pathSeparator,
  pickFolder,
  recentScanRoots,
  removeRecentScanRoot,
  scanRootWarning,
} from "../api.js";
import { el } from "../dom.js";
import { count, relativeTime, splitPath } from "../format.js";
import { icon } from "../icons.js";
import { noSessions, sessionList } from "../session-row.js";
import { undoSections } from "../outcomes.js";
import { showStatus } from "../status.js";

const LATEST_SESSIONS = 3;

export function render(root, nav) {
  const badge = el("button", { type: "button", className: "config-badge" });
  const configError = el("p", { className: "status error" });
  const tiles = el("div", { className: "tiles" });
  const status = el("p", { className: "status" });
  const sessions = el("div");
  const seeAll = el("button", { type: "button", className: "link", hidden: true });
  seeAll.addEventListener("click", () => nav.history(1));

  root.append(
    el("header", { className: "home-header" }, el("h1", {}, "Files Cleaner"), badge),
    configError,
    el("p", { className: "lead" }, "Pick a folder to scan. Nothing changes until you approve a Plan."),
    tiles,
    status,
    el("div", { className: "section-head" }, el("h2", {}, "Sessions"), seeAll),
    sessions,
  );

  let scanning = false;
  /** Scans `folder`, after the protected-path warning if it has one, then shows the review. */
  const startScan = async (folder) => {
    if (scanning) return;
    scanning = true;
    tiles.classList.add("busy");
    try {
      // A protected folder can still be scanned, once the user has read why it's protected.
      const warning = await scanRootWarning(folder);
      if (warning && !(await confirmScan(status, warning))) {
        status.replaceChildren();
      } else {
        showStatus(status, `Scanning ${folder}…`);
        await nav.steps.scanFolder(folder, (line) => showStatus(status, line));
        return;
      }
    } catch (error) {
      showStatus(status, describeError(error), true);
    }
    scanning = false;
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

}

/** The Settings button, which also says when the config has an error, spelt out below. */
async function showConfig(badge, errorLine) {
  try {
    await loadConfig();
    badge.classList.remove("error");
    badge.title = "Open Settings";
    badge.replaceChildren(icon("settings"), "Settings");
    errorLine.textContent = "";
  } catch (error) {
    badge.classList.add("error");
    badge.title = "Open Settings";
    badge.replaceChildren(icon("settings"), "Settings: config error");
    errorLine.textContent = describeError(error);
  }
}

/** A tile per Recent Scan Root, then the "Pick folder" tile. */
async function showTiles(node, startScan, pick) {
  const pickTile = el(
    "button",
    { type: "button", className: "tile pick-tile" },
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
  if (recent.length === 0) {
    node.append(el("p", { className: "tiles-empty" }, "Folders you scan show up here, so you can clean them again in one click."));
  }
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

/** The latest Sessions, with "See all" once there are any; an Undo's result shows above them. */
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
  node.replaceChildren(...(undoResult ? undoSections(undoResult) : []), list);
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
