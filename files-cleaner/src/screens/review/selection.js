// The Review screen's selection: files, and copies, are selected as in Finder, by clicking, by dragging a
// rectangle over them, or with the arrow keys (Shift adding to the selection), then trashed or put back (or
// kept, for copies) from the toolbar always shown above the list of files (not copies, whose pane has none),
// their right-click menu or ⌘⌫ (Ctrl+Delete elsewhere), or moved with "Move to…" in their right-click menu.
// With nothing selected in a Suggested Folder, the toolbar trashes the whole folder; elsewhere its buttons
// are disabled with nothing selected, and never hide, so the list never moves. A file opens in its default app by double-clicking it or from
// its right-click menu.
import { describeError, onAnotherDisk, openFile, pathSeparator, pickFolder } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { openEach } from "../../shared/plan/file-list.js";
import { plural } from "../../shared/format.js";
import { icon } from "../../shared/icons.js";
import { showStatus } from "../../shared/status.js";
import { marqueeSelect } from "./marquee.js";
import { clickSelection } from "./row-selection.js";
import { copiesText, displayName, fileName, UNSORTED } from "./pane-helpers.js";
import { folderTally } from "./review-choices.js";

/**
 * The selection of the Review screen with context `ctx`: `toolbar`, `draw()`, `selectable` and `copyMenu`
 * making a row selectable or openable, `move` moving files into a Suggested Folder, and `clearStatus()`
 * dropping the toolbar's error once another section is listed.
 */
export function selection(ctx) {
  const { state, view, menu, pane } = ctx;
  const keep = el("button", { type: "button", className: "small" }, "Keep");
  const trash = el("button", { type: "button", className: "small" }, icon("trash"), el("span", {}, "Trash"));
  const selected = el("span", { className: "muted review-toolbar-count" });
  // Why Move to… or opening a file failed, beside the toolbar's buttons.
  const status = el("p", { className: "status review-toolbar-status" });
  const toolbar = el("div", { className: "review-toolbar", role: "toolbar", ariaLabel: "Selected files" }, keep, trash, selected, status);
  /** Every selectable row now listed in the pane. */
  const rows = () => [...pane.querySelectorAll("[data-path]")];
  const copies = () => view.current === "duplicates";
  /** The Suggested Folder listed with nothing selected in it, which the toolbar's Trash then acts on, or null. */
  const wholeFolder = () => (view.selection.size === 0 && state.choices.includes(view.current) && view.current.files.size > 0 ? view.current : null);

  const move = (paths, to) => {
    state.move(paths, to);
    view.selection = new Set();
    ctx.redraw.pane();
  };

  /** Asks for any folder on the files' disk and moves `paths` there, through a Suggested Folder going into it; a folder on another disk is refused. */
  const moveToOther = async (paths) => {
    showStatus(status, "");
    try {
      const folder = await pickFolder("Pick a folder to move the files to");
      if (folder && (await onAnotherDisk(paths, folder))) {
        showStatus(status, `“${fileName(folder)}” is on another disk. Pick a folder on the same disk.`, true);
      } else if (folder) {
        state.moveToOther(paths, folder, pathSeparator);
        view.selection = new Set();
      }
    } catch (error) {
      showStatus(status, describeError(error), true);
    }
    ctx.redraw.pane();
  };

  /** Where the selected files can go from the pane: every folder but the one listed, or Unsorted. */
  const destinations = () => [...(view.current === "left" ? [] : [null]), ...state.choices.filter((choice) => choice !== view.current)];

  /** Whether the file at `path` in the listed section is marked Trash. */
  const isTrashed = (path) => {
    if (copies()) return state.duplicateOf.get(path).copies.get(path) === "trash";
    if (view.current === "left") return state.left.trashed.has(path);
    return view.current.files.get(path) === "trash";
  };

  /** Marks `paths` in the listed section Trash when `trashed`, or puts them back (keeps them, for copies), keeping the selection. */
  const setTrashed = (paths, trashed) => {
    for (const path of paths) {
      if (copies()) state.setCopy(state.duplicateOf.get(path), path, trashed ? "trash" : "keep");
      else if (view.current === "left") state.trashLeft(path, trashed);
      else state.setFile(view.current, path, trashed ? "trash" : "inFolder");
    }
    ctx.redraw.pane();
  };

  /** Trashes `paths`, or puts them all back when every one is trashed already. */
  const toggleTrash = (paths) => setTrashed(paths, !paths.every(isTrashed));

  keep.addEventListener("click", () => setTrashed([...view.selection], false));
  trash.addEventListener("click", () => {
    const folder = wholeFolder();
    if (folder) {
      state.trashAllFiles(folder, !folderTally(folder).trashed);
      ctx.redraw.pane();
    } else if (copies()) setTrashed([...view.selection], true);
    else toggleTrash([...view.selection]);
  });

  function clear() {
    view.selection = new Set();
    draw();
  }

  /** Marks the selected rows in the pane, and brings the toolbar up to date with what can be done with them. */
  function draw() {
    const listed = rows();
    const shown = new Set(listed.map((row) => row.dataset.path));
    view.selection = new Set([...view.selection].filter((path) => shown.has(path)));
    for (const row of listed) row.classList.toggle("selected", view.selection.has(row.dataset.path));
    const paths = [...view.selection];
    const none = paths.length === 0;
    const folder = wholeFolder();
    keep.hidden = !copies();
    keep.disabled = none;
    trash.disabled = none && !folder;
    if (folder) trash.lastChild.textContent = folderTally(folder).trashed ? "Put back folder" : "Trash folder";
    else trash.lastChild.textContent = !copies() && !none && paths.every(isTrashed) ? "Put back" : "Trash";
    selected.textContent = none ? "" : `${copies() ? copiesText(paths.length) : plural(paths.length, "file")} selected`;
  }

  /** The right-click menu's actions on `paths`, besides opening them: Keep and Trash for copies; Trash or Put back, and Move to…, for files. */
  function menuItems(paths, row, event) {
    if (copies()) {
      return [
        { label: "Keep", action: () => setTrashed(paths, false) },
        { label: "Trash", action: () => setTrashed(paths, true) },
      ];
    }
    const targets = [
      ...destinations().map((target) => ({ label: target ? displayName(target) : UNSORTED, action: () => move(paths, target) })),
      { label: "Other folder…", action: () => moveToOther(paths) },
    ];
    return [
      { label: paths.every(isTrashed) ? "Put back" : "Trash", action: () => toggleTrash(paths) },
      { label: "Move to…", action: () => menu.open([], row, event, targets) },
    ];
  }

  /** Opens the files at `paths` in their default apps, saying so if one can't be opened. */
  const openFiles = (paths) => openEach(paths, openFile).catch((error) => showStatus(status, describeError(error), true));

  /**
   * Makes `row`, listing the file at `path`, selectable as in Finder, opened by double-clicking it, with a
   * right-click menu acting on the selection, opening it or showing it in its folder, and, when `movable`,
   * draggable by its name onto a folder in the sidebar, taking the whole selection when it is in it. A copy
   * isn't movable: its fate moves it.
   */
  function selectable(row, path, { movable = true } = {}) {
    row.dataset.path = path;
    row.tabIndex = 0;
    const handle = row.querySelector(".folder-file-name, .duplicate-copy-folder");
    handle.draggable = movable;
    const select = ({ toggle, range }) => {
      const order = rows().map((node) => node.dataset.path);
      view.selection = clickSelection(view.selection, order, view.anchor, path, { toggle, range });
      if (!range) view.anchor = path;
      draw();
    };
    row.addEventListener("click", (event) => {
      if (event.target.closest("button, input")) return;
      select({ toggle: event.metaKey || event.ctrlKey, range: event.shiftKey });
    });
    row.addEventListener("keydown", (event) => {
      if (event.key !== " " || event.target !== row) return;
      event.preventDefault();
      select({ toggle: true, range: false });
    });
    row.addEventListener("dblclick", (event) => {
      if (!event.target.closest("button, input")) openFiles([path]);
    });
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      if (!view.selection.has(path)) select({ toggle: false, range: false });
      const paths = [...view.selection];
      menu.open(paths, row, event, menuItems(paths, row, event), () => openFiles(paths));
    });
    row.addEventListener("dragstart", (event) => {
      view.dragged = view.selection.has(path) ? [...view.selection] : [path];
      view.draggedFolder = null;
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", view.dragged.join("\n"));
      for (const node of rows()) {
        if (view.dragged.includes(node.dataset.path)) node.classList.add("dragging");
      }
    });
    row.addEventListener("dragend", () => {
      view.dragged = null;
      for (const node of pane.querySelectorAll(".dragging")) node.classList.remove("dragging");
    });
    return row;
  }

  /** Gives `row`, listing the Kept Copy at `path` that stays where it is, a right-click menu to open it or show it in the file manager, and opens it on a double-click. */
  function copyMenu(row, path) {
    row.addEventListener("dblclick", () => openFiles([path]));
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      menu.open([path], row, event, [], () => openFiles([path]));
    });
    return row;
  }

  // ⌘⌫ on macOS, Ctrl+Delete elsewhere, toggles Trash on the selection, as long as no field is being typed in
  // (a copy's Keep box isn't one).
  pane.addEventListener("keydown", (event) => {
    const shortcut = (event.metaKey && event.key === "Backspace") || (event.ctrlKey && event.key === "Delete");
    if (!shortcut || view.selection.size === 0 || event.target.closest("input:not(.duplicate-copy-keep), select, textarea")) return;
    event.preventDefault();
    toggleTrash([...view.selection]);
  });

  // The arrow keys select the row above or below, as in Finder; with Shift, from the last row clicked to it.
  pane.addEventListener("keydown", (event) => {
    if ((event.key !== "ArrowDown" && event.key !== "ArrowUp") || event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.target.closest("input, select, textarea, button")) return;
    const listed = rows();
    if (listed.length === 0) return;
    event.preventDefault();
    const order = listed.map((row) => row.dataset.path);
    const focused = event.target.closest("[data-path]")?.dataset.path;
    const from = order.indexOf(focused ?? view.anchor);
    const step = event.key === "ArrowDown" ? 1 : -1;
    const to = from === -1 ? (step === 1 ? 0 : order.length - 1) : Math.min(order.length - 1, Math.max(0, from + step));
    const path = order[to];
    if (event.shiftKey) {
      if (view.anchor === null || !order.includes(view.anchor)) view.anchor = order[Math.max(from, 0)];
      view.selection = clickSelection(view.selection, order, view.anchor, path, { range: true });
    } else {
      view.selection = new Set([path]);
      view.anchor = path;
    }
    listed[to].focus();
    draw();
  });

  marqueeSelect(pane, {
    rows,
    // A drag from a file's name moves the file.
    ignore: (target) => Boolean(target.closest("[draggable='true']")),
    select: (paths) => {
      view.selection = new Set(paths);
      view.anchor = paths[0] ?? null;
      draw();
    },
    // Clicking the toolbar's blank part isn't clicking empty space.
    ignoreClick: (target) => toolbar.contains(target),
    clear,
  });

  return { toolbar, draw, selectable, copyMenu, move, clearStatus: () => showStatus(status, "") };
}
