// The native menus, wired up: the menu bar's items stay in step with what applies on the screen shown, its
// clicks run there, and right-click menus are the OS's own. The app's items (New Scan, the screens, Help) come
// from main.js, the screen's from the screen; a screen's item replaces the app's of the same id.
import { onContextMenuChoice, onMenuClick, setMenuState, showContextMenu } from "../api.js";
import { contextMenu, menuBar } from "./menu-items.js";

const none = () => ({});
let appItems = none;
let screenItems = none;
// The right-click menu shown last: the item chosen in it reaches the app after it has closed.
let shown = null;
// What the backend was told last, so a focus change that changes nothing tells it nothing.
let sent = "";
let queued = false;

/** Whether a text field has the focus, which takes ⌘A, ⌘⌫ and Return for itself. */
const typing = () => Boolean(document.activeElement?.matches("textarea, input:not([type=checkbox], [type=radio], [type=button])"));

/** The menu bar for the items that apply now. */
const current = () =>
  menuBar(
    { ...appItems(), ...screenItems() },
    { typing: typing(), blocked: Boolean(document.querySelector("dialog[open]")), selectText: () => document.activeElement.select() },
  );

/** Tells the backend what applies now, once for every change made in the same task. */
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

/** Sets the app's items: `items()` returns them as `menuBar` takes them, worked out again on every change. */
export function setAppMenu(items) {
  appItems = items;
  refreshMenu();
}

/** Sets the screen's items as `setAppMenu` does, or none with `items` null. */
export function setScreenMenu(items) {
  screenItems = items ?? none;
  refreshMenu();
}

/** The screen's items now, to set again with `setScreenMenu` when the screen comes back. */
export const screenMenu = () => screenItems;

/** Shows `items`, as `contextMenu` in menu-items.js takes them, as the OS's own right-click menu at the pointer. */
export function showMenu(items) {
  shown = contextMenu(items);
  showContextMenu(shown.spec).catch(() => {
    // Nothing is shown; every action is still in the menu bar or on the screen.
  });
}

onMenuClick((id) => current().run(id));
onContextMenuChoice((place) => shown?.choose(place));
// Whether a text field or a sheet has the focus changes what applies.
document.addEventListener("focusin", refreshMenu);
document.addEventListener("focusout", refreshMenu);
