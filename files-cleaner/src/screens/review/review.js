// Review: the one screen after the scan, holding the whole Plan, laid out like Finder or Mail. A sidebar
// lists the Duplicates, then the Suggested Folders and New folder, then the files left where they are (shown
// as Unsorted), and the pane beside it lists the one picked. The Duplicate Groups show at once. The loose
// files are read in the background meanwhile, which needs the Grouping Models (without them, they're
// offered in the Suggested Folders' place), and the Suggested Folders are filled in once that's done.
// Rows have no buttons: files, and copies, are selected as in Finder, by clicking, by dragging a rectangle
// over them or with the arrow keys, then trashed or put back (or kept, for copies) from the toolbar always
// shown above the list, their right-click menu or ⌘⌫. Files are moved by dragging them by the name onto a
// folder in the sidebar, or with "Move to…" in their right-click menu, which also offers any other folder on
// disk. A file opens in its default app by double-clicking it or from its right-click menu. The sidebar
// counts the files in each item, and grays out a folder whose every file is trashed. It also adds a folder,
// and merges one into another by dragging it there, or trashes (with ⌘⌫ too) or removes one, from its
// right-click menu; the toolbar trashes the listed folder when nothing in it is selected. Nothing
// happens to the files until the header's button, which waits for the reading (it can be stopped early to
// group only what was read), carries out the whole Plan as one Session; Cancel stops the reading and goes Home.
//
// This module only lays the screen out and connects its parts, each in a module of its own: the folder
// panes, the Duplicates pane, the reading, the sidebar, the selection with the toolbar, and the header.
// They share one context, `ctx`, and never import one another; each part's own functions are reached
// through it, as `ctx.sessions`, `ctx.selection` and so on.
import { pathSeparator } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { revealMenu } from "../../shared/plan/file-list.js";
import { splitPath } from "../../shared/format.js";
import { duplicatesPane } from "./duplicates-pane.js";
import { folderPanes } from "./folder-panes.js";
import { FILES_SHOWN } from "./pane-helpers.js";
import { reading } from "./reading.js";
import { ReviewState } from "./review-state.js";
import { selection } from "./selection.js";
import { sessions } from "./sessions.js";
import { sidebar } from "./sidebar.js";

export async function render(root, nav, result, duplicateGroups) {
  const state = new ReviewState(result, duplicateGroups);
  const view = {
    // What the pane lists: "duplicates", "folders" (until they're ready), "left" (the files left where they
    // are), or a Suggested Folder's choice.
    current: state.duplicates.length > 0 ? "duplicates" : "folders",
    // The files selected in the pane, and the one last clicked without Shift.
    selection: new Set(),
    anchor: null,
    // The files being dragged, or the Suggested Folder, while a drag is on.
    dragged: null,
    draggedFolder: null,
    // The Suggested Folder being renamed in the sidebar, if any: `{ choice, node }`, its name field kept
    // across redraws of the sidebar, so the name from before and the focus stay.
    renaming: null,
    // Set while the sidebar is redrawn, when moving the name field can blur it.
    redrawing: false,
    duplicatesShown: FILES_SHOWN,
    // What the last "Keep only copies in" left alone, said beside it until the next pick.
    keepSkipped: "",
    // How far the reading is, shown in the sidebar while the files are read.
    readingProgress: "",
    // Whether the Session is running.
    running: false,
  };

  const show = (to) => {
    view.current = to;
    view.selection = new Set();
    view.anchor = null;
    ctx.selection.clearStatus();
    drawPane();
  };
  /**
   * What every part of the screen shares: the Review state and the view, the Scan Root's name, the
   * right-click menu, the pane, `show(section)` listing a section, and the redraws, among them
   * `section(name)`, redrawing the pane if it lists `name` and everything around it otherwise. Each part adds
   * its own functions to it below.
   */
  const ctx = {
    nav,
    state,
    view,
    rootName: splitPath(result.root, pathSeparator).name,
    menu: revealMenu(),
    pane: el("section", { className: "folder-pane" }),
    show,
    redraw: {
      pane: drawPane,
      sidebar: () => ctx.sidebar.draw(),
      header: () => ctx.sessions.drawHeader(),
      all: changed,
      section: (name) => (view.current === name ? drawPane() : changed()),
    },
  };
  Object.assign(ctx, {
    folders: folderPanes(ctx),
    duplicates: duplicatesPane(ctx),
    reading: reading(ctx),
    sidebar: sidebar(ctx),
    selection: selection(ctx),
    sessions: sessions(ctx, result),
  });

  root.append(ctx.sessions.node, el("div", { className: "folder-split" }, ctx.sidebar.node, ctx.pane), ctx.menu.node, ctx.reading.footer);
  drawPane();

  /** The pane: the listed section of the Plan, under the toolbar when it lists files. A toolbar button or a file's row that had focus keeps it. */
  function drawPane() {
    const { toolbar } = ctx.selection;
    const focused = toolbar.contains(document.activeElement) ? document.activeElement : null;
    const focusedPath = document.activeElement?.closest?.(".folder-pane [data-path]")?.dataset.path;
    if (state.choices.includes(view.current)) {
      const panel = ctx.folders.panel(view.current);
      ctx.pane.replaceChildren(toolbar, panel.node);
      panel.draw();
    } else {
      const sections = {
        duplicates: () => [toolbar, ...ctx.duplicates.panel()],
        folders: ctx.reading.waiting,
        left: () => [toolbar, ...ctx.folders.leftPanel()],
      };
      ctx.pane.replaceChildren(...sections[view.current]());
      changed();
    }
    if (focused && !focused.disabled) focused.focus();
    else if (focusedPath) ctx.pane.querySelector(`[data-path="${CSS.escape(focusedPath)}"]`)?.focus();
  }

  /** Brings the sidebar, the selection, the header and the Emptied Folders up to date after the pane changed. */
  function changed() {
    ctx.redraw.sidebar();
    ctx.selection.draw();
    ctx.redraw.header();
    ctx.sessions.refreshEmptied();
  }

  await ctx.reading.start();
}
