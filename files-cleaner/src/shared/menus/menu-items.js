// Apart from the backend that draws the menus, so it can be tested.

/**
 * The backend reports only an item's place ("1.0" for a submenu's first), so `choose` maps it
 * back.
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

export const IS_MAC = /Mac/.test(globalThis.navigator?.platform ?? "");

/** `[macOS, others]`. Must match the bindings in `menu.rs`, except the Rules screen's own. */
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

export const shortcutText = (name, mac = IS_MAC) => SHORTCUTS[name][mac ? 0 : 1];

export const withShortcut = (text, name, mac = IS_MAC) => `${text} ${shortcutText(name, mac)}`;

/** Ids must match `menu.rs`. */
export const MENU_BAR_ITEMS = ["newScan", "selectAll", "trash", "trashNow", "rename", "review", "rules", "history", "settings", "help"];

const MENU_BAR_LISTS = ["recent", "moveTo", "mergeInto"];

/** A text field needs these keys itself. */
const TYPING_TAKES = new Set(["trash", "trashNow", "rename"]);

/** In a text field Select All must select its text; with a sheet open nothing applies. */
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

/** Says how to select more until the user has done it once. */
export function selectionLine(selected, single, multiSelectSeen, mac = IS_MAC) {
  const hint = single && !multiSelectSeen ? `${mac ? "⇧ or ⌘" : "Shift or Ctrl"}-click to select more` : "";
  return [selected, hint].filter(Boolean).join(" · ");
}
