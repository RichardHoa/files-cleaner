// A screen's item replaces the app's of the same id.
import { onContextMenuChoice, onMenuClick, setMenuState, showContextMenu } from "./api.js";

const none = () => ({});
let appItems = none;
let screenItems = none;
// Kept, as the choice reaches the app after the menu has closed.
let shown = null;
// So a focus change that changes nothing isn't sent.
let sent = "";
let queued = false;

/** Text fields take ⌘A, ⌘⌫ and Return for themselves. */
const typing = () => Boolean(document.activeElement?.matches("textarea, input:not([type=checkbox], [type=radio], [type=button])"));

const current = () =>
  menuBar(
    { ...appItems(), ...screenItems() },
    { typing: typing(), blocked: Boolean(document.querySelector("dialog[open]")), selectText: () => document.activeElement.select() },
  );

/** Once for every change made in the same task, to save calls. */
export function refreshMenu() {
  if (queued) return;
  queued = true;
  queueMicrotask(() => {
    queued = false;
    const state = current().state;
    const text = JSON.stringify(state);
    if (text === sent) return;
    sent = text;
    setMenuState(state).catch(() => {
      // The menu bar stays as it was; every action is still on the screen.
      sent = "";
    });
  });
}

export function setAppMenu(items) {
  appItems = items;
  refreshMenu();
}

export function setScreenMenu(items) {
  screenItems = items ?? none;
  refreshMenu();
}

/** To set again when the screen comes back. */
export const screenMenu = () => screenItems;

export function showMenu(items) {
  shown = contextMenu(items);
  showContextMenu(shown.spec).catch(() => {
    // Every action is still in the menu bar or on the screen.
  });
}

onMenuClick((id) => current().run(id));
onContextMenuChoice((place) => shown?.choose(place));
// Whether a text field or a sheet has the focus changes what applies.
document.addEventListener("focusin", refreshMenu);
document.addEventListener("focusout", refreshMenu);

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
