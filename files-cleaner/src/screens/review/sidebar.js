// Reads like Finder's sidebar. Nothing in it is dragged, on purpose.
import { el, icon, showStatus } from "../../shared/dom.js";
import { count, plural } from "../../shared/format.js";
import { showMenu } from "../../shared/menus.js";
import { folderTally, nameMissing, trashedCopiesIn } from "./review-choices.js";
import { clickSelection, displayName, UNSORTED } from "./selection.js";

const WAITING_ON_USER = { offer: "Download models", ask: "Waiting for you" };

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

  const forgetFolder = (gone, next) => {
    if (view.renaming?.choice === gone) view.renaming = null;
    if (view.current === gone) ctx.show(next);
    else ctx.redraw.pane();
  };
  const merge = (from, into) => {
    state.merge(from, into);
    forgetFolder(from, into);
  };
  /** As in Finder: the whole selection when `choice` is in it, else `choice` alone. */
  const actedOn = (choice) => (view.picked.has(choice) ? state.choices.filter((other) => view.picked.has(other)) : [choice]);
  const pick = (choice, { toggle, range }) => {
    const picked = clickSelection(view.picked, state.choices, view.pickAnchor, choice, { toggle, range });
    if (picked.size === 0) return;
    if (!range) view.pickAnchor = choice;
    const [only] = picked;
    if (picked.size > 1) ctx.show("picked", picked);
    // Clicking the listed one again leaves the sidebar as it is, so a double-click lands on the same item.
    else if (only !== view.current) ctx.show(only);
  };
  /** Dropped folders may be the ones listed, so something else must be. */
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

  /** Redrawing drops the focus, so it is put back. */
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

  function foldersItem() {
    const needed = WAITING_ON_USER[state.phase];
    if (!needed) return sidebarItem("folders", state.phase === "reading" ? "Reading…" : "Suggested Folders", null);
    const item = sidebarItem("folders", needed, null);
    item.querySelector(".folder-sidebar-progress").replaceWith(el("span", { className: "folder-sidebar-dot", ariaHidden: "true" }));
    return item;
  }

  function showProgress(text) {
    view.readingProgress = text;
    const progress = node.querySelector(".folder-sidebar-progress");
    if (progress) progress.textContent = text;
  }

  function choiceItem(choice) {
    if (choice === view.renaming?.choice) return view.renaming.node;
    const { going, trashed } = folderTally(choice);
    const classes = [nameMissing(choice) ? "unnamed" : "", trashed ? "trashed" : ""];
    return sidebarItem(choice, displayName(choice), going, classes.filter(Boolean).join(" "));
  }

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
      // A redraw here would break a double-click.
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

  const mergeTargets = (choice) =>
    state.choices.filter((other) => other !== choice).map((into) => ({ label: displayName(into), action: () => merge(choice, into) }));

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

  function renameListed(choice) {
    if (view.current !== choice) ctx.show(choice);
    const item = [...node.children].find((child) => child.dataset.section === String(state.choices.indexOf(choice)));
    if (item) rename(choice, item);
  }

  /**
   * Rename only while focus is on the item or nowhere, so Return keeps its meaning everywhere
   * else.
   */
  function menuItems() {
    const choice = state.choices.includes(view.current) && !view.renaming ? view.current : null;
    if (!choice) return {};
    const focused = document.activeElement;
    const onItem = focused === document.body || focused?.dataset?.section === String(state.choices.indexOf(choice));
    return { rename: onItem ? () => renameListed(choice) : null, mergeInto: mergeTargets(choice) };
  }

  function rename(choice, item) {
    view.renaming = { choice, node: renameField(choice) };
    item.replaceWith(view.renaming.node);
    selectName();
  }

  function selectName() {
    const input = view.renaming.node.querySelector("input");
    input.focus();
    input.select();
  }

  /** An empty name is refused, as a folder can't be made without one. */
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
