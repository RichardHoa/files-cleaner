// One screen for the whole Plan. This module only lays it out: the parts share `ctx` and never
// import one another, so each can be read alone.
import { pathSeparator } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { plural, splitPath } from "../../shared/format.js";
import { seen } from "../../shared/introduction.js";
import { refreshMenu, setScreenMenu } from "../../shared/menus.js";
import { duplicatesPane } from "./duplicates-pane.js";
import { folderPanes } from "./folder-panes.js";
import { reading } from "./reading.js";
import { ReviewState } from "./review-state.js";
import { FILES_SHOWN, selection } from "./selection.js";
import { sessions } from "./sessions.js";
import { sidebar } from "./sidebar.js";

export async function render(root, appNav, result, duplicateGroups) {
  // After Cancel or Run, going back would show a Plan that no longer applies.
  const nav = {
    ...appNav,
    home: (...args) => (appNav.leaveReview(), appNav.home(...args)),
    session: (...args) => (appNav.leaveReview(), appNav.session(...args)),
  };
  const state = new ReviewState(result, duplicateGroups);
  const view = {
    // "duplicates", "folders" (until they're ready), "left", "picked" (several folders selected),
    // or a Suggested Folder's choice.
    current: state.duplicates.length > 0 ? "duplicates" : "folders",
    picked: new Set(),
    // Where a Shift-click selects from.
    pickAnchor: null,
    selection: new Set(),
    // Where a Shift-click selects from.
    anchor: null,
    // The field is kept across sidebar redraws, so its text and focus survive.
    renaming: null,
    // Moving the name field during a redraw can blur it, which must not end the rename.
    redrawing: false,
    duplicatesShown: FILES_SHOWN,
    readingProgress: "",
    running: false,
    // Assumed seen until loaded, so the hint doesn't flash.
    multiSelectSeen: true,
  };

  const show = (to, picked = state.choices.includes(to) ? new Set([to]) : new Set()) => {
    view.current = to;
    view.picked = picked;
    if (picked.size === 1) view.pickAnchor = to;
    view.selection = new Set();
    view.anchor = null;
    ctx.selection.clearStatus();
    drawPane();
  };
  /** Parts add their own functions below, which is how they reach one another. */
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

  /** While a Session runs, nothing that changes the Plan or leaves the screen applies. */
  function menuItems() {
    if (view.running) return { newScan: null, recent: [], rules: null, history: null, settings: null };
    const picked = state.choices.filter((choice) => view.picked.has(choice));
    const trashPicked = picked.length > 0 ? () => ctx.sessions.trashFoldersNow(picked) : null;
    const items = { ...ctx.selection.menuItems(), ...ctx.sidebar.menuItems() };
    if (view.current === "picked") items.trash = trashPicked;
    items.trashNow = view.current === "duplicates" ? ctx.duplicates.trashNow() : trashPicked;
    return items;
  }

  /** Redrawing drops the focus, so it is put back. */
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

  function changed() {
    ctx.redraw.sidebar();
    ctx.selection.draw();
    ctx.redraw.header();
    ctx.sessions.refreshEmptied();
    refreshMenu();
  }

  await ctx.reading.start();
}
