// Review: the one screen after the scan, holding the whole Plan, laid out like Mail. A sidebar
// lists the Duplicates, then the files left where they are and each Suggested Folder, then the Emptied
// Folders, and the pane beside it lists the one picked. The Duplicate Groups show at once. The loose
// files are read in the background meanwhile, which needs the Grouping Models (without them, they're
// offered in the Suggested Folders' place), and the Suggested Folders are filled in once that's done.
// The copies are trashed from the Duplicates pane, as a Session of their own, without waiting for the
// reading; likewise each Suggested Folder's moves and its trashes, and the trashes among the files left
// where they are, can be run from their pane on their own. Files, and copies, are selected as
// in Finder, by clicking or by dragging a rectangle over them, then trashed, put back or taken out of
// their folder (or kept, for copies) from the selection bar or their right-click menu. Files are moved by dragging them by the name onto a folder in the sidebar, from
// their right-click menu, or with "Move to…", which also offers any other folder on disk. A file opens in
// its default app by double-clicking it or from its right-click menu. The sidebar counts what goes into each
// folder, and grays out a folder whose every file is trashed. It also adds a folder, and merges one into
// another by dragging it there, or removes one, from its right-click menu. The header's button waits for the
// reading, which can be stopped early to group only what was read, then runs the rest of the Plan as one
// Session; Cancel stops the reading and goes Home, leaving whatever was already run, so it says Done once
// anything has.
//
// This module only lays the screen out and connects its parts, each in a module of its own: the folder
// panes, the Duplicates pane, the reading, the sidebar, the selection, and the Sessions with the header.
// They share one context, `ctx`, and never import one another; each part's own functions are reached
// through it, as `ctx.sessions`, `ctx.selection` and so on.
import { pathSeparator } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { revealMenu } from "../../shared/plan/file-list.js";
import { formatSize, plural, splitPath } from "../../shared/format.js";
import { icon } from "../../shared/icons.js";
import { duplicatesPane } from "./duplicates-pane.js";
import { folderPanes } from "./folder-panes.js";
import { FILES_SHOWN } from "./pane-helpers.js";
import { reading } from "./reading.js";
import { ReviewState } from "./review-state.js";
import { selection } from "./selection.js";
import { sessions } from "./sessions.js";
import { sidebar } from "./sidebar.js";
import { skippedChips } from "./skipped.js";

/** What the scan `result` left out of the Suggested Folders, as a row of chips, the detail in each one's tooltip. */
function leftOut(result) {
  return el(
    "p",
    { className: "skipped-chips" },
    el("span", { className: "muted" }, "Skipped:"),
    ...skippedChips(result).map(({ label, title }) => el("span", { className: "pill", title }, label)),
  );
}

export async function render(root, nav, result, duplicateGroups) {
  const totalSize = result.entries.reduce((sum, entry) => sum + entry.size, 0);
  const state = new ReviewState(result, duplicateGroups);
  const view = {
    // What the pane lists: "duplicates", "folders" (until they're ready), "left" (the files left where they
    // are), "emptied", or a Suggested Folder's choice.
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
    // What the last "Keep the copy in…" left alone, said under it until the next pick.
    keepSkipped: "",
    // Whether a Session is running.
    running: false,
  };

  const show = (to) => {
    view.current = to;
    view.selection = new Set();
    view.anchor = null;
    drawPane();
  };
  /**
   * What every part of the screen shares: the Review state and the view, the Scan Root's name, the status
   * line, the right-click menu, the pane, `show(section)` listing a section, and the redraws, among them
   * `section(name)`, redrawing the pane if it lists `name` and everything around it otherwise. Each part adds
   * its own functions to it below.
   */
  const ctx = {
    nav,
    state,
    view,
    rootName: splitPath(result.root, pathSeparator).name,
    status: el("p", { className: "status" }),
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
    sessions: sessions(ctx),
  });

  const { heading, run, scope, cancel } = ctx.sessions.header;
  root.append(
    el(
      "header",
      { className: "plan-head" },
      el(
        "div",
        {},
        heading,
        el("p", { className: "path root-path" }, icon("folder"), result.root),
        el(
          "p",
          { className: "lead" },
          `${plural(result.entries.length, "item")} · ${formatSize(totalSize)}`,
        ),
        leftOut(result),
      ),
      el("div", { className: "review-run" }, el("div", { className: "actions" }, cancel, run), scope),
    ),
    ctx.status,
    el("div", { className: "folder-split" }, ctx.sidebar.node, ctx.pane),
    ctx.menu.node,
    ctx.reading.footer,
  );
  drawPane();

  /** The pane: the listed section of the Plan. */
  function drawPane() {
    const bar = ctx.selection.bar;
    if (state.choices.includes(view.current)) {
      const panel = ctx.folders.panel(view.current);
      ctx.pane.replaceChildren(bar, panel.node);
      panel.draw();
      return;
    }
    const sections = {
      duplicates: () => [bar, ...ctx.duplicates.panel()],
      folders: ctx.reading.waiting,
      left: () => [bar, ...ctx.folders.leftPanel()],
      emptied: ctx.folders.emptiedPanel,
    };
    ctx.pane.replaceChildren(...sections[view.current]());
    changed();
  }

  /** Brings the sidebar, the selection, the header and the Emptied Folders up to date after the pane changed. */
  function changed() {
    ctx.redraw.sidebar();
    ctx.selection.draw();
    ctx.redraw.header();
    ctx.folders.refreshEmptied();
  }

  await ctx.reading.start();
}
