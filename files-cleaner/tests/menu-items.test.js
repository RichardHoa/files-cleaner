import assert from "node:assert/strict";
import { test } from "node:test";
import { contextMenu, menuBar, selectionLine, withShortcut } from "../src/shared/menus/menu-items.js";

test("a right-click menu is described to the backend by label, shortcut and enabled state, and the chosen id runs its action", () => {
  const ran = [];
  const menu = contextMenu([
    { label: "Trash", shortcut: "trash", action: () => ran.push("trash") },
    { label: "Move to…", submenu: [{ label: "Photos", action: () => ran.push("photos") }] },
    { separator: true },
    { label: "Open", enabled: false, action: () => ran.push("open") },
  ]);
  assert.deepEqual(menu.spec, [
    { label: "Trash", shortcut: "trash", enabled: true, submenu: null, separator: false },
    {
      label: "Move to…",
      shortcut: null,
      enabled: true,
      submenu: [{ label: "Photos", shortcut: null, enabled: true, submenu: null, separator: false }],
      separator: false,
    },
    { label: "", shortcut: null, enabled: true, submenu: null, separator: true },
    { label: "Open", shortcut: null, enabled: false, submenu: null, separator: false },
  ]);
  menu.choose("1.0");
  menu.choose("0");
  assert.deepEqual(ran, ["photos", "trash"]);
});

test("a tooltip ends with its shortcut, written with ⌘ on macOS and Ctrl on Windows and Linux", () => {
  assert.equal(withShortcut("Trash", "trash", true), "Trash ⌘⌫");
  assert.equal(withShortcut("Trash", "trash", false), "Trash Ctrl+Delete");
  assert.equal(withShortcut("Move Up", "moveUp", false), "Move Up Ctrl+Alt+↑");
});

test("the menu bar enables only what the screen offers, runs it by id, and leaves typing alone", () => {
  const ran = [];
  const items = {
    newScan: () => ran.push("newScan"),
    trash: () => ran.push("trash"),
    trashNow: null,
    selectAll: () => ran.push("selectAll"),
    moveTo: [{ label: "Photos", action: () => ran.push("photos") }],
  };
  const bar = menuBar(items, { selectText: () => ran.push("selectText") });
  assert.deepEqual(bar.state, { enabled: ["newScan", "selectAll", "trash"], recent: [], moveTo: ["Photos"], mergeInto: [] });
  bar.run("moveTo:0");
  bar.run("trash");
  bar.run("trashNow");

  // In a text field ⌘A selects its text and ⌘⌫ deletes in it, so Trash isn't offered.
  const typing = menuBar(items, { typing: true, selectText: () => ran.push("selectText") });
  assert.deepEqual(typing.state.enabled, ["newScan", "selectAll"]);
  typing.run("selectAll");
  typing.run("trash");
  assert.deepEqual(ran, ["photos", "trash", "selectText"]);
});

test("the selection line shows how to select more after one selection, until several have been selected once", () => {
  assert.equal(selectionLine("1 file selected", true, false, true), "1 file selected · ⇧ or ⌘-click to select more");
  assert.equal(selectionLine("", true, false, false), "Shift or Ctrl-click to select more");
  assert.equal(selectionLine("1 file selected", true, true, true), "1 file selected");
});
