// Pieces shared by the Plan and Session screens' file lists: the "Show more" links under a
// list, and the menu a right-click on a file opens.
import { describeError, revealInFileManager } from "./api.js";
import { el } from "./dom.js";
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

/**
 * A right-click menu with "Show in folder". `node` goes on the page once; `open(paths, row, event)`
 * opens it for the `contextmenu` `event` on `row`, and Show in folder selects `paths` in the file manager.
 */
export function revealMenu() {
  const menu = el("div", { className: "row-menu file-menu", role: "menu", hidden: true });
  const close = () => {
    if (menu.hidden) return;
    menu.hidden = true;
    document.removeEventListener("pointerdown", closeOutside, true);
    document.removeEventListener("keydown", closeOnEscape);
    window.removeEventListener("scroll", close);
  };
  const closeOutside = (event) => {
    if (!menu.contains(event.target)) close();
  };
  const closeOnEscape = (event) => {
    if (event.key === "Escape") close();
  };
  function open(paths, row, event) {
    const reveal = el("button", { type: "button", role: "menuitem" }, "Show in folder");
    reveal.addEventListener("click", () =>
      revealInFileManager(paths).then(close, (error) => menu.replaceChildren(el("p", { className: "status error" }, describeError(error)))),
    );
    menu.replaceChildren(reveal);
    menu.hidden = false;
    // At the pointer, kept inside the window; opened from the keyboard, under the row.
    const fromKeyboard = event.clientX === 0 && event.clientY === 0;
    const anchor = row.getBoundingClientRect();
    const x = fromKeyboard ? anchor.left : event.clientX;
    const y = fromKeyboard ? anchor.bottom : event.clientY;
    menu.style.left = `${Math.max(0, Math.min(x, window.innerWidth - menu.offsetWidth - 4))}px`;
    menu.style.top = `${Math.max(0, Math.min(y, window.innerHeight - menu.offsetHeight - 4))}px`;
    document.addEventListener("pointerdown", closeOutside, true);
    document.addEventListener("keydown", closeOnEscape);
    window.addEventListener("scroll", close);
    reveal.focus();
  }
  return { node: menu, open };
}
