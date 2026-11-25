import { askToConfirm, revealInFileManager } from "../api.js";
import { count } from "../format.js";
import { el } from "../dom.js";
import { PAGE_SIZE } from "./plan-view.js";

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
