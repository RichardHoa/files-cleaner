// Review: the one screen after the scan, holding the whole Plan, laid out like Finder or Mail. A sidebar
// lists the Duplicates, then the Suggested Folders and New folder, then the files left where they are (shown
// as Unsorted), and the pane beside it lists the one picked. The Duplicate Groups show at once. The loose
// files are read in the background meanwhile, which needs the Grouping Models (without them, they're
// offered in the Suggested Folders' place); the Rules' folders show at once, and the other Suggested Folders
// are added once that's done.
// Rows have no buttons: files, and copies, are selected as in Finder, by clicking or with the arrow keys,
// then trashed or put back (or kept, for copies) from the toolbar always shown above the list of files, their
// right-click menu or Edit › Trash (⌘⌫). The Duplicates pane has no toolbar: its Keep boxes choose, and its own button
// trashes the unticked copies at once, apart from the rest of the Plan. Files are moved with "Move to…" in
// their right-click menu, which also offers any other folder on disk; nothing on the screen is dragged. A file
// opens in its default app by double-clicking it or from its right-click menu. The sidebar counts the files in
// each item, and grays out a folder whose every file is trashed. Its Suggested Folders are selected as files
// are; it adds a folder, and merges one into another, trashes the selected ones' files now (with Edit › Trash Now,
// ⇧⌘⌫, too) or removes one, from its right-click menu. The toolbar over a Suggested Folder trashes its files now too. Those
// Trash Nows, and the copies', are Sessions of their own; nothing else happens to the files until the
// header's button, which waits for the reading (it can be stopped early to group only what was read), carries
// out the whole Plan as one Session; Cancel stops the reading and goes Home.
//
// This module only lays the screen out and connects its parts, each in a module of its own: the folder
// panes, the Duplicates pane, the reading, the sidebar, the selection with the toolbar, and the header.
// They share one context, `ctx`, and never import one another; each part's own functions are reached
// through it, as `ctx.sessions`, `ctx.selection` and so on. Every right-click menu is the OS's own, and the menu
// bar's Edit items act on what's selected here (see `menuItems`).
import { pathSeparator } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { plural, splitPath } from "../../shared/format.js";
import { refreshMenu, setScreenMenu } from "../../shared/menus/menus.js";
import { seen } from "../../shared/onboarding/seen.js";
import { duplicatesPane } from "./duplicates-pane.js";
import { folderPanes } from "./folder-panes.js";
import { FILES_SHOWN } from "./pane-helpers.js";
import { reading } from "./reading.js";
import { ReviewState } from "./review-state.js";
import { selection } from "./selection.js";
import { sessions } from "./sessions.js";
import { sidebar } from "./sidebar.js";

export async function render(root, appNav, result, duplicateGroups) {
  // Cancel and Run leave the Review for good: nothing goes back to it.
  const nav = {
    ...appNav,
    home: (...args) => (appNav.leaveReview(), appNav.home(...args)),
    session: (...args) => (appNav.leaveReview(), appNav.session(...args)),
  };
  const state = new ReviewState(result, duplicateGroups);
  const view = {
    // What the pane lists: "duplicates", "folders" (until they're ready), "left" (the files left where they
    // are), "picked" (several Suggested Folders selected, none listed), or a Suggested Folder's choice.
    current: state.duplicates.length > 0 ? "duplicates" : "folders",
    // The Suggested Folders selected in the sidebar, and the one last clicked there without Shift.
    picked: new Set(),
    pickAnchor: null,
    // The files selected in the pane, and the one last clicked without Shift.
    selection: new Set(),
    anchor: null,
    // The Suggested Folder being renamed in the sidebar, if any: `{ choice, node }`, its name field kept
    // across redraws of the sidebar, so the name from before and the focus stay.
    renaming: null,
    // Set while the sidebar is redrawn, when moving the name field can blur it.
    redrawing: false,
    duplicatesShown: FILES_SHOWN,
    // How far the reading is, shown in the sidebar while the files are read.
    readingProgress: "",
    // Whether the Session is running.
    running: false,
    // Whether the user has once selected several items, after which the selection line no longer says how.
    multiSelectSeen: true,
  };

  /** Lists `to`, with `picked` the Suggested Folders selected in the sidebar: `to` alone when it is one. */
  const show = (to, picked = state.choices.includes(to) ? new Set([to]) : new Set()) => {
    view.current = to;
    view.picked = picked;
    if (picked.size === 1) view.pickAnchor = to;
    view.selection = new Set();
    view.anchor = null;
    ctx.selection.clearStatus();
    drawPane();
  };
  /**
   * What every part of the screen shares: the Review state and the view, the Scan Root's name, the pane, `show(section, picked)` listing a section, and the redraws, among them
   * `section(name)`, redrawing the pane if it lists `name` and everything around it otherwise. Each part adds
   * its own functions to it below.
   */
  const ctx = {
    nav,
    state,
    view,
    rootName: splitPath(result.root, pathSeparator).name,
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

  root.append(ctx.sessions.node, el("div", { className: "folder-split" }, ctx.sidebar.node, ctx.pane), ctx.reading.footer);
  setScreenMenu(menuItems);
  drawPane();
  seen().then(({ multiSelectSeen }) => {
    view.multiSelectSeen = Boolean(multiSelectSeen);
    ctx.selection.draw();
  });

  /**
   * The menu bar's items for what's selected: the selection's and the sidebar's, then Trash Now on the selected
   * Suggested Folders, or the copies marked Trash when the Duplicates are listed, and Trash on several folders
   * selected, as ⌘⌫ there before. While a Session runs, nothing that changes the Plan or leaves the screen applies.
   */
  function menuItems() {
    if (view.running) return { newScan: null, recent: [], rules: null, history: null, settings: null };
    const picked = state.choices.filter((choice) => view.picked.has(choice));
    const trashPicked = picked.length > 0 ? () => ctx.sessions.trashFoldersNow(picked) : null;
    const items = { ...ctx.selection.menuItems(), ...ctx.sidebar.menuItems() };
    if (view.current === "picked") items.trash = trashPicked;
    items.trashNow = view.current === "duplicates" ? ctx.duplicates.trashNow() : trashPicked;
    return items;
  }

  /** The pane: the listed section of the Plan, under the toolbar when it lists files (not copies). A toolbar button or a file's row that had focus keeps it. */
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
        duplicates: ctx.duplicates.panel,
        folders: ctx.reading.waiting,
        left: () => [toolbar, ...ctx.folders.leftPanel()],
        picked: () => [el("p", { className: "note" }, `${plural(view.picked.size, "folder")} selected`)],
      };
      ctx.pane.replaceChildren(...sections[view.current]());
      changed();
    }
    if (focused && !focused.disabled) focused.focus();
    else if (focusedPath) ctx.pane.querySelector(`[data-path="${CSS.escape(focusedPath)}"]`)?.focus();
  }

  /** Brings the sidebar, the selection, the header, the menu bar and the Emptied Folders up to date after the pane changed. */
  function changed() {
    ctx.redraw.sidebar();
    ctx.selection.draw();
    ctx.redraw.header();
    ctx.sessions.refreshEmptied();
    refreshMenu();
  }

  await ctx.reading.start();
}
