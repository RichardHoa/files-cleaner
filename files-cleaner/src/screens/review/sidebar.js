// The Review screen's sidebar, read like Finder's: the Duplicates, then a Suggested Folders section with each
// folder and New folder, then the files left where they are, as Unsorted, each with how many files are in it.
// Suggested Folders are selected as in Finder, by clicking, ⌘-clicking and ⇧-clicking; one selected is listed.
// It adds a folder, renames one, or merges one into another, trashes the selected folders' files now or leaves
// those not marked Trash unsorted, from its right-click menu (the listed folder's ⋯ menu has the same items), and the Edit menu renames, merges and trashes now too (Trash Now, ⇧⌘⌫). Nothing
// in it is dragged. While the Suggested Folders wait on the user, their item says what's needed beside a blue dot.
import { el } from "../../shared/dom.js";
import { count, plural } from "../../shared/format.js";
import { icon } from "../../shared/icons.js";
import { showMenu } from "../../shared/menus/menus.js";
import { showStatus } from "../../shared/status.js";
import { folderTally, nameMissing, trashedCopiesIn } from "./review-choices.js";
import { clickSelection } from "./row-selection.js";
import { displayName, UNSORTED } from "./pane-helpers.js";

/** What the Suggested Folders need from the user, by their phase, while they wait on it. */
const WAITING_ON_USER = { offer: "Download models", ask: "Waiting for you" };

/**
 * The sidebar of the Review screen with context `ctx`: `node`, `draw()` bringing it up to date,
 * `showProgress(text)` saying how far the reading is, `menuItems()`, its part of the menu bar, and
 * `folderMenuItems(choice)`, a Suggested Folder's menu.
 */
export function sidebar(ctx) {
  const { state, view } = ctx;
  const node = el("nav", { className: "folder-sidebar", ariaLabel: "Plan" });
  const newFolder = el(
    "button",
    { type: "button", className: "folder-sidebar-item folder-sidebar-new" },
    icon("plus"),
    el("span", { className: "folder-sidebar-name" }, "New folder"),
  );
  newFolder.addEventListener("click", () => {
    const choice = state.addFolder();
    view.renaming = { choice, node: renameField(choice) };
    ctx.show(choice);
    selectName();
  });

  /** Forgets the Suggested Folder `gone`, just dropped, listing `next` if it was the one listed. */
  const forgetFolder = (gone, next) => {
    if (view.renaming?.choice === gone) view.renaming = null;
    if (view.current === gone) ctx.show(next);
    else ctx.redraw.pane();
  };
  const merge = (from, into) => {
    state.merge(from, into);
    forgetFolder(from, into);
  };
  /** The Suggested Folders a right-click or ⌘⌫ on `choice` acts on: the sidebar's selection when `choice` is in it, else `choice` alone. */
  const actedOn = (choice) => (view.picked.has(choice) ? state.choices.filter((other) => view.picked.has(other)) : [choice]);
  /** Selects the Suggested Folder `choice` after a click, Cmd- or Shift-click, listing it when it's the only one selected. */
  const pick = (choice, { toggle, range }) => {
    const picked = clickSelection(view.picked, state.choices, view.pickAnchor, choice, { toggle, range });
    if (picked.size === 0) return;
    if (!range) view.pickAnchor = choice;
    const [only] = picked;
    if (picked.size > 1) ctx.show("picked", picked);
    // Clicking the listed one again leaves the sidebar as it is, so a double-click lands on the same item.
    else if (only !== view.current) ctx.show(only);
  };
  /**
   * Leaves the files of `chosen`, Suggested Folders, not marked Trash unsorted. Those left with none go: Unsorted
   * is listed in place of the one listed, or of the several selected once fewer than two are left.
   */
  const leaveUnsorted = (chosen) => {
    for (const choice of chosen) state.leaveUnsorted(choice);
    const gone = chosen.filter((choice) => !state.choices.includes(choice));
    const picked = new Set([...view.picked].filter((choice) => !gone.includes(choice)));
    if (view.current === "picked") ctx.show(picked.size > 1 ? "picked" : "left", picked.size > 1 ? picked : new Set());
    else if (gone.includes(view.current)) forgetFolder(view.current, "left");
    else {
      view.picked = picked;
      ctx.redraw.pane();
    }
  };

  /**
   * The Plan in order: Duplicates, counting the copies that leave, then each Suggested Folder, counting the
   * files going into it, and New folder (or one item showing the reading until they're ready), then
   * Unsorted, counting its files not trashed. Clicking one lists it. The item that had focus keeps it.
   */
  function draw() {
    const focused = [...node.children].indexOf(document.activeElement);
    const typing = view.renaming?.node.contains(document.activeElement);
    view.redrawing = true;
    node.replaceChildren(
      sidebarItem("duplicates", "Duplicates", trashedCopiesIn(state.duplicates), "", "copies"),
      el("h2", { className: "folder-sidebar-section" }, "Suggested Folders"),
      ...(state.phase === "ready"
        ? [
            ...state.choices.map(choiceItem),
            // The rest of the files are still read, beside the Rules' folders.
            ...(state.more ? [sidebarItem("folders", state.more === "reading" ? "Reading…" : "Reading failed", null)] : []),
            newFolder,
            sidebarItem("left", UNSORTED, state.left.size - state.left.trashed.size, "", "files"),
          ]
        : [foldersItem()]),
    );
    view.redrawing = false;
    node.children[focused]?.focus();
    if (typing) view.renaming.node.querySelector("input").focus();
  }

  /**
   * The item standing for the Suggested Folders until they're ready: the reading, or, while they wait on
   * the user, what's needed beside a blue dot.
   */
  function foldersItem() {
    const needed = WAITING_ON_USER[state.phase];
    if (!needed) return sidebarItem("folders", state.phase === "reading" ? "Reading…" : "Suggested Folders", null);
    const item = sidebarItem("folders", needed, null);
    item.querySelector(".folder-sidebar-progress").replaceWith(el("span", { className: "folder-sidebar-dot", ariaHidden: "true" }));
    return item;
  }

  /** Shows `text`, how far the reading is, on the item standing for the Suggested Folders until they're ready. */
  function showProgress(text) {
    view.readingProgress = text;
    const progress = node.querySelector(".folder-sidebar-progress");
    if (progress) progress.textContent = text;
  }

  /** The sidebar item for the Suggested Folder `choice`, or its name field while it's renamed. */
  function choiceItem(choice) {
    if (choice === view.renaming?.choice) return view.renaming.node;
    const { going, trashed } = folderTally(choice);
    const classes = [nameMissing(choice) ? "unnamed" : "", trashed ? "trashed" : ""];
    return sidebarItem(choice, displayName(choice), going, classes.filter(Boolean).join(" "));
  }

  /**
   * A sidebar item listing `section` with `n` items, or no count when `n` is null. A Suggested Folder's
   * is selected along with others by ⌘- or ⇧-clicking it, renamed by double-clicking it, or Return once
   * it's listed, as in Finder, trashed now with Edit › Trash Now, and has a right-click menu.
   */
  function sidebarItem(section, label, n, className = "", iconName = "folder") {
    const item = el(
      "button",
      { type: "button", className: `folder-sidebar-item ${className}`, ariaCurrent: String(section === view.current || view.picked.has(section)), title: label },
      icon(iconName),
      el("span", { className: "folder-sidebar-name" }, label),
      section === "folders"
        ? el("span", { className: "muted folder-sidebar-count folder-sidebar-progress" }, view.readingProgress)
        : el("span", { className: "muted folder-sidebar-count" }, n == null ? "" : count(n)),
    );
    if (state.choices.includes(section)) item.dataset.section = String(state.choices.indexOf(section));
    item.addEventListener("click", (event) => {
      if (state.choices.includes(section)) pick(section, { toggle: event.metaKey || event.ctrlKey, range: event.shiftKey });
      // Clicking the listed one again leaves the sidebar as it is.
      else if (section !== view.current) ctx.show(section);
    });
    if (state.choices.includes(section)) {
      item.addEventListener("dblclick", () => rename(section, item));
      item.addEventListener("keydown", (event) => {
        if (event.key !== "Enter" || section !== view.current) return;
        event.preventDefault();
        rename(section, item);
      });
      item.addEventListener("contextmenu", (event) => folderMenu(section, item, event));
    }
    return item;
  }

  /** The other Suggested Folders `choice` can be merged into, as menu items. */
  const mergeTargets = (choice) =>
    state.choices.filter((other) => other !== choice).map((into) => ({ label: displayName(into), action: () => merge(choice, into) }));

  /**
   * The menu of the Suggested Folder `choice`, from its right-click or its pane's ⋯: Rename, "Merge into…" listing
   * the other folders, "Trash all N files now" and "Leave N files unsorted", counting those not marked Trash.
   */
  function folderMenuItems(choice) {
    const others = mergeTargets(choice);
    const { going } = folderTally(choice);
    return [
      { label: "Rename", shortcut: "rename", action: () => renameListed(choice) },
      { label: "Merge into…", enabled: others.length > 0, submenu: others },
      { separator: true },
      {
        label: `Trash all ${plural(choice.files.size, "file")} now`,
        shortcut: "trashNow",
        enabled: choice.files.size > 0 && !view.running,
        action: () => ctx.sessions.trashFoldersNow([choice]),
      },
      { label: `Leave ${plural(going, "file")} unsorted`, enabled: going > 0, action: () => leaveUnsorted([choice]) },
    ];
  }

  /** The right-click menu of the Suggested Folder `choice`: its `folderMenuItems`, or, on one of several selected, what acts on them all. */
  function folderMenu(choice, item, event) {
    event.preventDefault();
    const chosen = actedOn(choice);
    if (chosen.length === 1) return showMenu(folderMenuItems(choice));
    const folders = plural(chosen.length, "folder");
    showMenu([
      { label: `Trash ${folders} now`, shortcut: "trashNow", enabled: !view.running, action: () => ctx.sessions.trashFoldersNow(chosen) },
      { label: `Leave files of ${folders} unsorted`, enabled: chosen.some((other) => folderTally(other).going > 0), action: () => leaveUnsorted(chosen) },
    ]);
  }

  /** Lists `choice`, if it isn't already, and renames it in the sidebar. */
  function renameListed(choice) {
    if (view.current !== choice) ctx.show(choice);
    const item = [...node.children].find((child) => child.dataset.section === String(state.choices.indexOf(choice)));
    if (item) rename(choice, item);
  }

  /**
   * The sidebar's part of the menu bar, with one Suggested Folder listed: Rename, while the focus is on its item
   * or nowhere, so Return keeps its meaning everywhere else, and Merge Into….
   */
  function menuItems() {
    const choice = state.choices.includes(view.current) && !view.renaming ? view.current : null;
    if (!choice) return {};
    const focused = document.activeElement;
    const onItem = focused === document.body || focused?.dataset?.section === String(state.choices.indexOf(choice));
    return { rename: onItem ? () => renameListed(choice) : null, mergeInto: mergeTargets(choice) };
  }

  /** Turns the sidebar item `item` of `choice` into a name field. */
  function rename(choice, item) {
    view.renaming = { choice, node: renameField(choice) };
    item.replaceWith(view.renaming.node);
    selectName();
  }

  /** Puts the focus in the name field being renamed, with the name selected. */
  function selectName() {
    const input = view.renaming.node.querySelector("input");
    input.focus();
    input.select();
  }

  /**
   * The name field renaming `choice` in the sidebar. The pane shows the name as it's typed; Return or
   * clicking away saves it and Esc puts back the name from before. An empty name is refused: Return
   * says so, and clicking away puts back the name from before.
   */
  function renameField(choice) {
    const before = choice.name;
    const input = el("input", { type: "text", className: "folder-sidebar-rename", value: choice.name, ariaLabel: "Folder name", spellcheck: false });
    const error = el("p", { className: "status error folder-name-error" });
    const field = el("div", { className: "folder-sidebar-renaming" }, el("div", { className: "folder-sidebar-item" }, icon("folder"), input), error);
    const named = () => {
      ctx.folders.drawName(choice);
      ctx.redraw.header();
    };
    const end = (save) => {
      // Already ended: replacing the field can blur it.
      if (view.renaming?.choice !== choice) return;
      if (!save) state.rename(choice, before);
      view.renaming = null;
      const hadFocus = field.contains(document.activeElement);
      const item = choiceItem(choice);
      field.replaceWith(item);
      if (hadFocus) item.focus();
      named();
    };
    input.addEventListener("input", () => {
      state.rename(choice, input.value);
      showStatus(error, "");
      input.ariaInvalid = "false";
      named();
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        end(false);
      } else if (event.key === "Enter") {
        event.preventDefault();
        if (input.value.trim()) return end(true);
        showStatus(error, "A folder needs a name.", true);
        input.ariaInvalid = "true";
      }
    });
    input.addEventListener("blur", () => view.redrawing || end(input.value.trim() !== ""));
    return field;
  }

  return { node, draw, showProgress, menuItems, folderMenuItems };
}
