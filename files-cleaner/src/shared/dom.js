import { askToConfirm, describeError, revealInFileManager } from "./api.js";
import { count, PAGE_SIZE } from "./format.js";

export function el(tag, props = {}, ...children) {
  const node = Object.assign(document.createElement(tag), props);
  node.append(...children);
  return node;
}

// Inline, as there is no icon library (ADR-0003).
const SVG = "http://www.w3.org/2000/svg";

const FOLDER = "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z";

const PATHS = {
  trash: "M4 7h16 M9 7V4.5h6V7 M6 7l1 13h10l1-13 M10 11v5 M14 11v5",
  move: "M4 12h15 M13 6l6 6-6 6",
  rename: "M4 20h4L19 9l-4-4L4 16z M13.5 6.5l4 4",
  createFolder: `${FOLDER} M12 10v6 M9 13h6`,
  folder: FOLDER,
  copies: "M8 8h11v12H8z M5 16V4h11",
  files: "M8 3h7l4 4v11H8z M15 3v4h4 M5 7v14h11",
  more: "M6 12a1 1 0 1 1-2 0a1 1 0 1 1 2 0 M13 12a1 1 0 1 1-2 0a1 1 0 1 1 2 0 M20 12a1 1 0 1 1-2 0a1 1 0 1 1 2 0",
  plus: "M12 5v14 M5 12h14",
  up: "M6 15l6-6 6 6",
  down: "M6 9l6 6 6-6",
  alert: "M10.3 3.9 2.4 17.5a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z M12 9v4 M12 17h.01",
  close: "M6 6l12 12 M18 6L6 18",
  settings:
    "M4 7h9 M17 7h3 M4 12h3 M11 12h9 M4 17h11 M19 17h1 M17 7a2 2 0 1 1-4 0a2 2 0 1 1 4 0 M11 12a2 2 0 1 1-4 0a2 2 0 1 1 4 0 M19 17a2 2 0 1 1-4 0a2 2 0 1 1 4 0",
};

/** Decorative: say what it means in text beside it. */
export function icon(name, className = "") {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", `icon ${className}`.trim());
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG, "path");
  path.setAttribute("d", PATHS[name]);
  svg.append(path);
  return svg;
}

// Glides for about as long as the update took to come, so steady progress moves without pauses.

/** In ms, so it never falls far behind. */
const MAX_GLIDE = 1500; // ms

export function progressBar(className = "") {
  const fill = el("div", { className: "progress-fill" });
  const node = el("div", { className: `progress indeterminate ${className}`.trim(), role: "progressbar" }, fill);
  node.ariaValueMin = "0";
  node.ariaValueMax = "100";
  let last = null;

  return {
    node,
    set(value, max) {
      if (!(max > 0)) {
        node.classList.add("indeterminate");
        node.removeAttribute("aria-valuenow");
        last = null;
        return;
      }
      const fraction = Math.min(Math.max(value / max, 0), 1);
      const now = performance.now();
      // A bar starting out, or going back for a new phase, jumps there.
      const glide = last === null || fraction < last.fraction ? 0 : Math.min(now - last.at, MAX_GLIDE);
      node.classList.remove("indeterminate");
      fill.style.transitionDuration = `${glide}ms`;
      fill.style.transform = `scaleX(${fraction})`;
      node.ariaValueNow = String(Math.round(fraction * 100));
      last = { fraction, at: now };
    },
  };
}

export function showStatus(node, text, isError = false) {
  node.textContent = text;
  node.classList.toggle("error", isError);
  node.classList.remove("warning-block", "progress-line");
}

/** Replaces the progress: a whole step failed, not just one action. */
export function showWarning(node, text) {
  showStatus(node, text);
  node.classList.add("warning-block");
}

/** Buttons are disabled so the action can't be started twice. */
export async function busy(node, buttons, label, action) {
  for (const button of buttons) button.disabled = true;
  showStatus(node, label);
  try {
    return await action();
  } catch (error) {
    showStatus(node, describeError(error), true);
    return undefined;
  } finally {
    for (const button of buttons) button.disabled = false;
  }
}

export function drawMore(node, total, shown, setLimit) {
  node.replaceChildren();
  if (shown >= total) return;
  node.append(el("span", { className: "muted" }, `Showing ${shown} of ${total}`));
  // With a page or less left, "Show 50 more" would do the same as "Show all".
  if (total - shown > PAGE_SIZE) {
    const page = el("button", { type: "button", className: "link" }, `Show ${PAGE_SIZE} more`);
    page.addEventListener("click", () => setLimit(shown + PAGE_SIZE));
    node.append(page);
  }
  const all = el("button", { type: "button", className: "link" }, `Show all ${total}`);
  all.addEventListener("click", () => setLimit(Infinity));
  node.append(all);
}

/** More at once could flood the screen with windows. */
const OPEN_WITHOUT_ASKING = 5;

/** Asks first when there are many. Rejects with the first error. */
export async function openEach(items, openOne) {
  if (items.length > OPEN_WITHOUT_ASKING && !(await askToConfirm(`Open ${count(items.length)} files?`, "Open"))) return;
  await Promise.all(items.map(openOne));
}

export function fileMenuItems(paths, items = [], openAll = null, onError = () => {}) {
  const below = [
    ...(openAll ? [{ label: "Open", action: openAll }] : []),
    ...(paths.length > 0 ? [{ label: "Show in folder", action: () => revealInFileManager(paths).catch(onError) }] : []),
  ];
  return [...items, ...(items.length > 0 && below.length > 0 ? [{ separator: true }] : []), ...below];
}
