// The Review screen's selection: files, and copies, are selected as in Finder, by clicking or with the arrow
// keys (Shift adding to the selection), then marked Trash or put back (or kept, for copies) from the toolbar
// always shown above the list of files (not copies, whose pane has none), their right-click menu or Edit › Trash
// (⌘⌫, Ctrl+Delete elsewhere), or moved with the toolbar's Move to…, their right-click menu or Edit › Move To….
// Moving takes only the files not marked Trash, and "Leave unsorted" is apart from the folders, as it makes none.
// The toolbar is always shown, so nothing below it moves; it holds only what acts on the selection, its buttons
// shown once files are selected. What acts on a whole Suggested Folder is in its ⋯ menu (see the folder panes). Its
// count line says how to select more until the user has once. A file opens in its default app by double-clicking
// it or from its right-click menu. Nothing is dragged.
import { describeError, onAnotherDisk, openFile, pathSeparator, pickFolder } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { fileMenuItems, openEach } from "../../shared/plan/file-list.js";
import { plural } from "../../shared/format.js";
import { refreshMenu, showMenu } from "../../shared/menus/menus.js";
import { selectionLine, withShortcut } from "../../shared/menus/menu-items.js";
import { updateSeen } from "../../shared/onboarding/seen.js";
import { showStatus } from "../../shared/status.js";
import { clickSelection } from "./row-selection.js";
import { copiesText, displayName, fileName, LEAVE_UNSORTED } from "./pane-helpers.js";

/**
 * The selection of the Review screen with context `ctx`: `toolbar`, `draw()`, `selectable` and `copyMenu`
 * making a row selectable or openable, `move` moving files into a Suggested Folder, `clearStatus()`
 * dropping the toolbar's error once another section is listed, and `menuItems()`, its part of the menu bar.
 */
export function selection(ctx) {
  const { state, view, pane } = ctx;
  const keep = el("button", { type: "button", className: "small" }, "Keep");
  const trash = el("button", { type: "button", className: "small" }, el("span", {}, "Mark as Trash"));
  const moveTo = el("button", { type: "button", className: "small", ariaHasPopup: "menu" }, "Move to…");
  const selected = el("span", { className: "muted review-toolbar-count" });
  // Why Move to… or opening a file failed, beside the toolbar's buttons.
  const status = el("p", { className: "status review-toolbar-status" });
  const toolbar = el("div", { className: "review-toolbar", role: "toolbar", ariaLabel: "Selected files" }, keep, trash, moveTo, selected, status);
  /** Every selectable row now listed in the pane. */
  const rows = () => [...pane.querySelectorAll("[data-path]")];
  const copies = () => view.current === "duplicates";

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

  moveTo.addEventListener("click", () => showMenu(moveTargets([...view.selection])));
  keep.addEventListener("click", () => setTrashed([...view.selection], false));
  trash.addEventListener("click", () => {
    if (copies()) setTrashed([...view.selection], true);
    else toggleTrash([...view.selection]);
  });

  function clear() {
    view.selection = new Set();
    draw();
  }

  /** Marks the selected rows in the pane, and brings the toolbar and the menu bar up to date with what can be done with them. */
  function draw() {
    const listed = rows();
    const shown = new Set(listed.map((row) => row.dataset.path));
    view.selection = new Set([...view.selection].filter((path) => shown.has(path)));
    for (const row of listed) row.classList.toggle("selected", view.selection.has(row.dataset.path));
    const paths = [...view.selection];
    const none = paths.length === 0;
    keep.hidden = !copies() || none;
    trash.hidden = none;
    moveTo.hidden = copies() || none;
    moveTo.disabled = state.movable(paths) === 0;
    moveTo.title = moveTo.disabled ? "Files marked Trash aren't moved" : moveLabel(paths);
    trash.lastChild.textContent = copies() ? "Trash" : paths.every(isTrashed) ? "Put back" : "Mark as Trash";
    trash.title = withShortcut(trash.lastChild.textContent, "trash");
    if ((paths.length > 1 || view.picked.size > 1) && !view.multiSelectSeen) {
      view.multiSelectSeen = true;
      updateSeen((seen) => ({ ...seen, multiSelectSeen: true }));
    }
    const count = none ? "" : `${copies() ? copiesText(paths.length) : plural(paths.length, "file")} selected`;
    selected.textContent = selectionLine(count, paths.length === 1 || (none && view.picked.size === 1), view.multiSelectSeen);
    refreshMenu();
  }

  /** "Move 4 files to…", counting only those of `paths` not marked Trash, which are all that move. */
  const moveLabel = (paths) => `Move ${plural(state.movable(paths), "file")} to…`;

  /**
   * Where `paths` can be moved: every Suggested Folder but the one listed, then any other folder on disk, then,
   * apart from the folders and unless Unsorted is listed, leaving them unsorted.
   */
  const moveTargets = (paths) => [
    ...state.choices.filter((choice) => choice !== view.current).map((target) => ({ label: displayName(target), action: () => move(paths, target) })),
    { label: "Other folder…", action: () => moveToOther(paths) },
    ...(view.current === "left" ? [] : [{ separator: true }, { label: LEAVE_UNSORTED, action: () => move(paths, null) }]),
  ];

  /** The right-click menu's actions on `paths`, besides opening them: Keep and Trash for copies; Trash or Put back, and Move N files to…, for files. */
  function rowMenuItems(paths) {
    if (copies()) {
      return [
        { label: "Keep", action: () => setTrashed(paths, false) },
        { label: "Trash", shortcut: "trash", action: () => setTrashed(paths, true) },
      ];
    }
    return [
      { label: paths.every(isTrashed) ? "Put back" : "Trash", shortcut: "trash", action: () => toggleTrash(paths) },
      { label: moveLabel(paths), enabled: state.movable(paths) > 0, submenu: moveTargets(paths) },
    ];
  }

  /**
   * The selection's part of the menu bar: Select All while rows are listed, Trash and Move To… while rows are
   * selected, as their right-click menu has them: Trash marks copies Trash and toggles it on files, and Move To… is
   * for files only, while one of them isn't marked Trash.
   */
  function menuItems() {
    const paths = [...view.selection];
    return {
      selectAll: rows().length > 0 ? selectAll : null,
      trash: paths.length === 0 ? null : copies() ? () => setTrashed(paths, true) : () => toggleTrash(paths),
      moveTo: paths.length > 0 && !copies() && state.movable(paths) > 0 ? moveTargets(paths).filter((item) => !item.separator) : [],
    };
  }

  /** Selects every row listed, so Mark as Trash then marks a whole folder. */
  function selectAll() {
    view.selection = new Set(rows().map((row) => row.dataset.path));
    draw();
  }

  /** Opens the files at `paths` in their default apps, saying so if one can't be opened. */
  const openFiles = (paths) => openEach(paths, openFile).catch((error) => showStatus(status, describeError(error), true));
  /** Says in the toolbar's status line why a right-click menu's action failed. */
  const showError = (error) => showStatus(status, describeError(error), true);

  /**
   * Makes `row`, listing the file at `path`, selectable as in Finder, opened by double-clicking it, with a
   * right-click menu acting on the selection, opening it or showing it in its folder.
   */
  function selectable(row, path) {
    row.dataset.path = path;
    row.tabIndex = 0;
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
      showMenu(fileMenuItems(paths, rowMenuItems(paths), () => openFiles(paths), showError));
    });
    return row;
  }

  /** Gives `row`, listing the Kept Copy at `path` that stays where it is, a right-click menu to open it or show it in the file manager, and opens it on a double-click. */
  function copyMenu(row, path) {
    row.addEventListener("dblclick", () => openFiles([path]));
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      showMenu(fileMenuItems([path], [], () => openFiles([path]), showError));
    });
    return row;
  }

  // ⌘⌫ (Ctrl+Delete elsewhere) and ⌘A (Ctrl+A) are the menu bar's Trash and Select All (see `menuItems`).

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

  // A plain click on empty space, not on a row, a control or the toolbar's blank part, deselects.
  pane.addEventListener("click", (event) => {
    const target = event.target;
    if (target.closest("button, input, select, a, label, [contenteditable]") || toolbar.contains(target)) return;
    if (!rows().some((row) => row.contains(target))) clear();
  });

  return { toolbar, draw, selectable, copyMenu, move, clearStatus: () => showStatus(status, ""), menuItems };
}
