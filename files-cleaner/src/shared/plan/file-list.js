// Pieces shared by the review and Session screens' file lists: the "Show more" links under a
// list, the menu a right-click on a file opens, and opening files in their default apps.
import { askToConfirm, revealInFileManager } from "../api.js";
import { count } from "../format.js";
import { el } from "../dom.js";
import { PAGE_SIZE } from "./plan-view.js";

/**
 * Fills `node` with "Showing 50 of 120", "Show 50 more" and "Show all 120" while `shown` of
 * `total` files are listed; empties it once they all are. `setLimit(n)` lists up to `n`.
 */
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

/** How many files can be opened at once without asking first. */
const OPEN_WITHOUT_ASKING = 5;

/**
 * Opens each of `items` with `openOne(item)`, in the app the OS picks for it, asking first when there are
 * more than a few. Rejects with the first error.
 */
export async function openEach(items, openOne) {
  if (items.length > OPEN_WITHOUT_ASKING && !(await askToConfirm(`Open ${count(items.length)} files?`, "Open"))) return;
  await Promise.all(items.map(openOne));
}

/**
 * A right-click menu's items, as `showMenu` takes them: `items` above "Open" and "Show in folder". Show in folder
 * selects `paths` in the file manager, saying why it couldn't with `onError(error)`, and is left out when `paths`
 * is empty. Open runs `openAll()`, and is left out without it.
 */
export function fileMenuItems(paths, items = [], openAll = null, onError = () => {}) {
  const below = [
    ...(openAll ? [{ label: "Open", action: openAll }] : []),
    ...(paths.length > 0 ? [{ label: "Show in folder", action: () => revealInFileManager(paths).catch(onError) }] : []),
  ];
  return [...items, ...(items.length > 0 && below.length > 0 ? [{ separator: true }] : []), ...below];
}
