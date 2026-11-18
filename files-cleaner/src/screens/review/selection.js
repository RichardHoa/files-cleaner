// The Review screen's selection: files, and copies, are selected as in Finder, by clicking or by dragging a
// rectangle over them, then trashed, put back or taken out of their folder (or kept, for copies) from the
// selection bar or their right-click menu, or moved with "Move to…". A file opens in its default app by
// double-clicking it or from its right-click menu.
import { describeError, onAnotherDisk, openFile, pathSeparator, pickFolder } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { openEach } from "../../shared/plan/file-list.js";
import { plural } from "../../shared/format.js";
import { showStatus } from "../../shared/status.js";
import { marqueeSelect } from "./marquee.js";
import { clickSelection } from "./row-selection.js";
import { copiesText, displayName, fileName } from "./pane-helpers.js";

/**
 * The selection of the Review screen with context `ctx`: `bar`, the selection bar, `draw()`, `selectable` and
 * `copyMenu` making a row selectable or openable, and `move` moving files into a Suggested Folder.
 */
export function selection(ctx) {
  const { state, view, status, menu, pane } = ctx;
  const bar = el("div", { className: "selection-bar", hidden: true });
  /** Every selectable row now listed in the pane. */
  const rows = () => [...pane.querySelectorAll("[data-path]")];

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
        showStatus(status, `“${fileName(folder)}” is on a different disk, and the app only moves items within one disk. Pick a folder on the same disk.`, true);
      } else if (folder) {
        state.moveToOther(paths, folder, pathSeparator);
        view.selection = new Set();
      }
    } catch (error) {
      showStatus(status, describeError(error), true);
    }
    ctx.redraw.pane();
  };

  /** Where the selected files can go from the pane: every folder but the one listed and those whose moves ran, or back where they are. */
  const destinations = () => [...(view.current === "left" ? [] : [null]), ...state.choices.filter((choice) => choice !== view.current && !choice.moved)];

  function clear() {
    view.selection = new Set();
    draw();
  }

  /** Marks the selected rows in the pane, and shows what can be done with them. */
  function draw() {
    const listed = rows();
    const shown = new Set(listed.map((row) => row.dataset.path));
    view.selection = new Set([...view.selection].filter((path) => shown.has(path)));
    for (const row of listed) row.classList.toggle("selected", view.selection.has(row.dataset.path));
    bar.hidden = view.selection.size === 0;
    if (view.selection.size === 0) return;
    // A file already run, or a copy of a Duplicate Group that can't change any more, can only be opened.
    const paths = [...view.selection].filter((path) => !state.ran.has(path) && !(state.duplicateOf.has(path) && state.locked(state.duplicateOf.get(path))));
    const actions = (paths.length > 0 ? fileActions(paths) : []).map(({ label, action }) => {
      const button = el("button", { type: "button", className: "small" }, label);
      button.addEventListener("click", action);
      return button;
    });
    const deselect = el("button", { type: "button", className: "link" }, "Deselect");
    deselect.addEventListener("click", clear);
    const n = view.selection.size;
    const what = view.current === "duplicates" ? copiesText(n) : plural(n, "file");
    const movable = view.current !== "duplicates" && paths.length > 0;
    bar.replaceChildren(el("span", {}, `${what} selected`), ...actions, ...(movable ? [moveSelect(paths)] : []), deselect);
  }

  /** The "Move to…" menu for the files at `paths`: the other folders, back where they are, or any other folder on disk. */
  function moveSelect(paths) {
    const targets = destinations();
    const to = el(
      "select",
      { ariaLabel: "Move the selected files" },
      el("option", { value: "", disabled: true, selected: true }, "Move to…"),
      ...targets.map((target, i) => el("option", { value: i }, target ? displayName(target) : "Leave where they are")),
      el("option", { value: "other" }, "Other folder…"),
    );
    to.value = "";
    to.addEventListener("change", () => {
      if (to.value === "other") moveToOther(paths);
      else move(paths, targets[Number(to.value)]);
    });
    return to;
  }

  /**
   * What can be done to the files at `paths` in the listed section, besides moving them, as `[{ label, action }]`:
   * Keep or Trash for copies; Trash, or Put back once they're all trashed, for the files left where they are;
   * and for a Suggested Folder's, Trash, Take out and Put back, each unless every file is that already. The selection stays.
   */
  function fileActions(paths) {
    const each = (change) => () => {
      for (const path of paths) change(path);
      ctx.redraw.pane();
    };
    if (view.current === "duplicates") {
      const fate = (value) => each((path) => state.setCopy(state.duplicateOf.get(path), path, value));
      return [
        { label: "Keep", action: fate("keep") },
        { label: "Trash", action: fate("trash") },
      ];
    }
    if (view.current === "left") {
      if (paths.every((path) => state.left.trashed.has(path))) return [{ label: "Put back", action: each((path) => state.trashLeft(path, false)) }];
      return [{ label: "Trash", action: each((path) => state.trashLeft(path, true)) }];
    }
    const folder = view.current;
    return [
      ["Trash", "trash"],
      ["Take out", "takeOut"],
      ...(folder.moved ? [] : [["Put back", "inFolder"]]),
    ]
      .filter(([, file]) => !paths.every((path) => folder.files.get(path) === file))
      .map(([label, file]) => ({ label, action: each((path) => state.setFile(folder, path, file)) }));
  }

  /** Opens the files at `paths` in their default apps, saying so if one can't be opened. */
  const openFiles = (paths) => openEach(paths, openFile).catch((error) => showStatus(status, describeError(error), true));

  /**
   * Makes `row`, listing the file at `path`, selectable as in Finder, opened by double-clicking it, with a
   * right-click menu to open the selection or show it in its folder, and, when `movable`, draggable by its
   * name onto a folder in the sidebar, taking the whole selection when it is in it. A copy isn't movable: its
   * fate moves it.
   */
  function selectable(row, path, { movable = true } = {}) {
    row.dataset.path = path;
    row.tabIndex = 0;
    const handle = row.querySelector(".folder-file-name, .duplicate-copy-name");
    handle.draggable = movable;
    const select = ({ toggle, range }) => {
      const order = rows().map((node) => node.dataset.path);
      view.selection = clickSelection(view.selection, order, view.anchor, path, { toggle, range });
      if (!range) view.anchor = path;
      draw();
    };
    row.addEventListener("click", (event) => {
      if (event.target.closest("button")) return;
      select({ toggle: event.metaKey || event.ctrlKey, range: event.shiftKey });
    });
    row.addEventListener("keydown", (event) => {
      if (event.key !== " " || event.target !== row) return;
      event.preventDefault();
      select({ toggle: true, range: false });
    });
    row.addEventListener("dblclick", (event) => {
      if (!event.target.closest("button")) openFiles([path]);
    });
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      if (!view.selection.has(path)) select({ toggle: false, range: false });
      const paths = [...view.selection];
      menu.open(paths, row, event, [], () => openFiles(paths));
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

  marqueeSelect(pane, {
    rows,
    // A drag from a file's name moves the file.
    ignore: (target) => Boolean(target.closest("[draggable='true']")),
    select: (paths) => {
      view.selection = new Set(paths);
      view.anchor = paths[0] ?? null;
      draw();
    },
    // Clicking the selection bar's blank part isn't clicking empty space.
    ignoreClick: (target) => bar.contains(target),
    clear,
  });

  return { bar, draw, selectable, copyMenu, move };
}
