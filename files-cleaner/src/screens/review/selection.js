// The toolbar is always shown, so nothing below it moves. Moving takes only the files not marked
// Trash.
import { describeError, onAnotherDisk, openFile, pathSeparator, pickFolder } from "../../shared/api.js";
import { el, fileMenuItems, icon, openEach, showStatus } from "../../shared/dom.js";
import { count, plural, splitPath } from "../../shared/format.js";
import { updateSeen } from "../../shared/introduction.js";
import { refreshMenu, selectionLine, showMenu, withShortcut } from "../../shared/menus.js";

export function selection(ctx) {
  const { state, view, pane } = ctx;
  const keep = el("button", { type: "button", className: "small" }, "Keep");
  const trash = el("button", { type: "button", className: "small" }, el("span", {}, "Mark as Trash"));
  const moveTo = el("button", { type: "button", className: "small", ariaHasPopup: "menu" }, "Move to…");
  const selected = el("span", { className: "muted review-toolbar-count" });
  const status = el("p", { className: "status review-toolbar-status" });
  const toolbar = el("div", { className: "review-toolbar", role: "toolbar", ariaLabel: "Selected files" }, keep, trash, moveTo, selected, status);
  const rows = () => [...pane.querySelectorAll("[data-path]")];
  const copies = () => view.current === "duplicates";

  const move = (paths, to) => {
    state.move(paths, to);
    view.selection = new Set();
    ctx.redraw.pane();
  };

  /** Goes through a Suggested Folder, so the move is part of the Plan. Another disk is refused. */
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

  const isTrashed = (path) => {
    if (copies()) return state.duplicateOf.get(path).copies.get(path) === "trash";
    if (view.current === "left") return state.left.trashed.has(path);
    return view.current.files.get(path) === "trash";
  };

  /** Keeps the selection, so the user can change their mind at once. */
  const setTrashed = (paths, trashed) => {
    for (const path of paths) {
      if (copies()) state.setCopy(state.duplicateOf.get(path), path, trashed ? "trash" : "keep");
      else if (view.current === "left") state.trashLeft(path, trashed);
      else state.setFile(view.current, path, trashed ? "trash" : "inFolder");
    }
    ctx.redraw.pane();
  };

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

  /** Counts only what would really move: files not marked Trash. */
  const moveLabel = (paths) => `Move ${plural(state.movable(paths), "file")} to…`;

  /** "Leave unsorted" comes last and apart, as it makes no folder. */
  const moveTargets = (paths) => [
    ...state.choices.filter((choice) => choice !== view.current).map((target) => ({ label: displayName(target), action: () => move(paths, target) })),
    { label: "Other folder…", action: () => moveToOther(paths) },
    ...(view.current === "left" ? [] : [{ separator: true }, { label: LEAVE_UNSORTED, action: () => move(paths, null) }]),
  ];

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

  function menuItems() {
    const paths = [...view.selection];
    return {
      selectAll: rows().length > 0 ? selectAll : null,
      trash: paths.length === 0 ? null : copies() ? () => setTrashed(paths, true) : () => toggleTrash(paths),
      moveTo: paths.length > 0 && !copies() && state.movable(paths) > 0 ? moveTargets(paths).filter((item) => !item.separator) : [],
    };
  }

  function selectAll() {
    view.selection = new Set(rows().map((row) => row.dataset.path));
    draw();
  }

  const openFiles = (paths) => openEach(paths, openFile).catch((error) => showStatus(status, describeError(error), true));
  const showError = (error) => showStatus(status, describeError(error), true);

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

  /** A Kept Copy must stay where it is, so there is nothing else to offer. */
  function copyMenu(row, path) {
    row.addEventListener("dblclick", () => openFiles([path]));
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      showMenu(fileMenuItems([path], [], () => openFiles([path]), showError));
    });
    return row;
  }

  // ⌘⌫ and ⌘A are bound by the native menu bar (see `menuItems`), so they aren't handled here.

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

  // As in Finder, a click on empty space deselects.
  pane.addEventListener("click", (event) => {
    const target = event.target;
    if (target.closest("button, input, select, a, label, [contenteditable]") || toolbar.contains(target)) return;
    if (!rows().some((row) => row.contains(target))) clear();
  });

  return { toolbar, draw, selectable, copyMenu, move, clearStatus: () => showStatus(status, ""), menuItems };
}

// The way Finder selects rows, which users expect on this screen.

/** `anchor` is the item last clicked without Shift. */
export function clickSelection(selection, order, anchor, item, { toggle = false, range = false } = {}) {
  const from = anchor === null ? -1 : order.indexOf(anchor);
  if (range && from !== -1) {
    const to = order.indexOf(item);
    const [start, end] = from < to ? [from, to] : [to, from];
    return new Set([...(toggle ? selection : []), ...order.slice(start, end + 1)]);
  }
  if (toggle) {
    const next = new Set(selection);
    if (next.has(item)) next.delete(item);
    else next.add(item);
    return next;
  }
  return new Set([item]);
}

// The panes reach each other only through the Review screen's context, so the pieces they share are
// imported from here.

export const FILES_SHOWN = 50;

export const fileName = (path) => splitPath(path, pathSeparator).name;

export const displayName = (choice) => choice.name.trim() || "No name";

export const UNSORTED = "Unsorted";

export const LEAVE_UNSORTED = "Leave unsorted";

export const trashedMark = () => el("span", { className: "folder-file-trashed", role: "img", ariaLabel: "Trashed", title: "Trashed" }, icon("trash"));

export const copiesText = (n) => `${count(n)} ${n === 1 ? "copy" : "copies"}`;
