// A screen's item replaces the app's of the same id.
import { onContextMenuChoice, onMenuClick, setMenuState, showContextMenu } from "../api.js";
import { contextMenu, menuBar } from "./menu-items.js";

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
