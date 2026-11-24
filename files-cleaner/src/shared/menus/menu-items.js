// What the native menus show, worked out apart from the backend that draws them: the right-click menus'
// items, the menu bar's items that apply, and each shortcut as the OS writes it.

/**
 * A right-click menu of `items`, each `{ label, action, shortcut, enabled, submenu }` or `{ separator: true }`,
 * `shortcut` naming one of `SHORTCUTS`. `spec` is what the backend draws; `choose(id)` runs the action of the
 * item the backend reports chosen, `id` being its place, "1.0" for a submenu's first item.
 */
export function contextMenu(items) {
  const describe = (list) =>
    list.map((item) => ({
      label: item.label ?? "",
      shortcut: item.shortcut ?? null,
      enabled: item.enabled ?? true,
      submenu: item.submenu ? describe(item.submenu) : null,
      separator: item.separator ?? false,
    }));
  const choose = (id) => {
    let list = items;
    let item = null;
    for (const index of id.split(".").map(Number)) {
      item = list?.[index];
      list = item?.submenu;
    }
    item?.action?.();
  };
  return { spec: describe(items), choose };
}

/** Whether the app runs on macOS, whose shortcuts use ⌘ where Windows and Linux use Ctrl. */
export const IS_MAC = /Mac/.test(globalThis.navigator?.platform ?? "");

/**
 * Each shortcut as macOS and as Windows and Linux write it. The menu bar's are bound by the backend (see
 * `menu.rs`); the Rules screen's by the screen itself.
 */
export const SHORTCUTS = {
  newScan: ["⌘N", "Ctrl+N"],
  selectAll: ["⌘A", "Ctrl+A"],
  trash: ["⌘⌫", "Ctrl+Delete"],
  trashNow: ["⇧⌘⌫", "Ctrl+Shift+Delete"],
  rename: ["↩", "Enter"],
  settings: ["⌘,", "Ctrl+,"],
  edit: ["↩", "Enter"],
  toggle: ["Space", "Space"],
  moveUp: ["⌥⌘↑", "Ctrl+Alt+↑"],
  moveDown: ["⌥⌘↓", "Ctrl+Alt+↓"],
  deleteRule: ["⌘⌫", "Delete"],
};

/** The shortcut `name` as this OS writes it. */
export const shortcutText = (name, mac = IS_MAC) => SHORTCUTS[name][mac ? 0 : 1];

/** A tooltip: `text` ending with the shortcut `name`, as this OS writes it. */
export const withShortcut = (text, name, mac = IS_MAC) => `${text} ${shortcutText(name, mac)}`;

/** The menu bar's items that act on their own, by the backend's id for each (see `menu.rs`). */
export const MENU_BAR_ITEMS = ["newScan", "selectAll", "trash", "trashNow", "rename", "review", "rules", "history", "settings", "help"];

/** The menu bar's submenus listing items of their own, each `{ label, action }`. */
const MENU_BAR_LISTS = ["recent", "moveTo", "mergeInto"];

/** What a text field takes for itself: in one, Trash, Trash Now and Rename don't apply. */
const TYPING_TAKES = new Set(["trash", "trashNow", "rename"]);

/**
 * The menu bar for `items`, the action of each of `MENU_BAR_ITEMS` that applies (null or left out when it
 * doesn't) and each of `MENU_BAR_LISTS`' items. `typing` is whether a text field has the focus, where Select All
 * runs `selectText()`; `blocked` whether a sheet is open, when nothing applies. `state` is what the backend greys
 * out and lists; `run(id)` runs the item the backend reports clicked, "moveTo:1" for a list's second.
 */
export function menuBar(items, { typing = false, blocked = false, selectText = () => {} } = {}) {
  const action = (id) => {
    if (blocked || (typing && TYPING_TAKES.has(id))) return null;
    if (typing && id === "selectAll") return selectText;
    const [list, index] = id.split(":");
    if (index !== undefined) return MENU_BAR_LISTS.includes(list) ? (items[list]?.[Number(index)]?.action ?? null) : null;
    return MENU_BAR_ITEMS.includes(id) ? (items[id] ?? null) : null;
  };
  const listed = (list) => (blocked ? [] : (items[list] ?? []).map((item) => item.label));
  return {
    state: {
      enabled: MENU_BAR_ITEMS.filter((id) => action(id)),
      recent: listed("recent"),
      moveTo: listed("moveTo"),
      mergeInto: listed("mergeInto"),
    },
    run: (id) => action(id)?.(),
  };
}

/**
 * The selection count line: `selected` ("1 file selected", or "" with only a sidebar folder selected), then, when
 * `single` (exactly one row or sidebar folder is selected) and the user hasn't yet selected several items
 * (`multiSelectSeen`), how to select more.
 */
export function selectionLine(selected, single, multiSelectSeen, mac = IS_MAC) {
  const hint = single && !multiSelectSeen ? `${mac ? "⇧ or ⌘" : "Shift or Ctrl"}-click to select more` : "";
  return [selected, hint].filter(Boolean).join(" · ");
}
