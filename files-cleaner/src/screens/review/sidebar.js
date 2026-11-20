// The Review screen's sidebar, read like Finder's: the Duplicates, then a Suggested Folders section with each
// folder and New folder, then the files left where they are, as Unsorted, each with how many files are in it.
// It adds a folder, renames one, merges one into another by dragging it there, or trashes it (all its files,
// also with ⌘⌫) or removes one, from its right-click menu, and takes the files dragged onto it.
import { el } from "../../shared/dom.js";
import { count } from "../../shared/format.js";
import { icon } from "../../shared/icons.js";
import { showStatus } from "../../shared/status.js";
import { folderTally, nameMissing, trashedCopiesIn } from "./review-choices.js";
import { displayName, UNSORTED } from "./pane-helpers.js";

/** The sidebar of the Review screen with context `ctx`: `node`, `draw()` bringing it up to date, and `showProgress(text)` saying how far the reading is. */
export function sidebar(ctx) {
  const { state, view, menu } = ctx;
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
  /** Trashes every file of `choice`, or puts them all back when every one is trashed already. */
  const toggleTrash = (choice) => {
    state.trashAllFiles(choice, !folderTally(choice).trashed);
    ctx.redraw.section(choice);
  };
  const remove = (choice) => {
    state.remove(choice);
    forgetFolder(choice, "left");
  };

  /**
   * The Plan in order: Duplicates, counting the copies that leave, then each Suggested Folder, counting the
   * files going into it, and New folder (or one item showing the reading until they're ready), then
   * Unsorted, counting its files not trashed. Clicking one lists it, and a file dragged onto a folder goes
   * into it. The item that had focus keeps it.
   */
  function draw() {
    const focused = [...node.children].indexOf(document.activeElement);
    const typing = view.renaming?.node.contains(document.activeElement);
    view.redrawing = true;
    node.replaceChildren(
      sidebarItem("duplicates", "Duplicates", trashedCopiesIn(state.duplicates), "", "copies"),
      el("h2", { className: "folder-sidebar-section" }, "Suggested Folders"),
      ...(state.phase === "ready"
        ? [...state.choices.map(choiceItem), newFolder, sidebarItem("left", UNSORTED, state.left.size - state.left.trashed.size)]
        : [sidebarItem("folders", state.phase === "reading" ? "Reading…" : "Suggested Folders", null)]),
    );
    view.redrawing = false;
    node.children[focused]?.focus();
    if (typing) view.renaming.node.querySelector("input").focus();
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
   * is renamed by double-clicking it, or Return once it's listed, as in Finder, merged by dragging it onto
   * another, and has a right-click menu; files can be dragged onto it, and onto Unsorted.
   */
  function sidebarItem(section, label, n, className = "", iconName = "folder") {
    const item = el(
      "button",
      { type: "button", className: `folder-sidebar-item ${className}`, ariaCurrent: String(section === view.current), title: label },
      icon(iconName),
      el("span", { className: "folder-sidebar-name" }, label),
      section === "folders"
        ? el("span", { className: "muted folder-sidebar-count folder-sidebar-progress" }, view.readingProgress)
        : el("span", { className: "muted folder-sidebar-count" }, n == null ? "" : count(n)),
    );
    // Clicking the listed one again leaves the sidebar as it is, so a double-click lands on the same item.
    item.addEventListener("click", () => section !== view.current && ctx.show(section));
    if (state.choices.includes(section)) {
      item.addEventListener("dblclick", () => rename(section, item));
      item.addEventListener("keydown", (event) => {
        if ((event.metaKey && event.key === "Backspace") || (event.ctrlKey && event.key === "Delete")) {
          event.preventDefault();
          return toggleTrash(section);
        }
        if (event.key !== "Enter" || section !== view.current) return;
        event.preventDefault();
        rename(section, item);
      });
      item.draggable = true;
      item.addEventListener("dragstart", (event) => {
        view.draggedFolder = section;
        view.dragged = null;
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", section.name);
      });
      item.addEventListener("dragend", () => {
        view.draggedFolder = null;
      });
      item.addEventListener("contextmenu", (event) => folderMenu(section, item, event));
      dropTarget(item, section);
    } else if (section === "left") dropTarget(item, null);
    return item;
  }

  /** The right-click menu of the Suggested Folder `choice`: "Merge into…", which then lists the other folders, Trash (or Put back) and Remove. */
  function folderMenu(choice, item, event) {
    event.preventDefault();
    const others = state.choices.filter((other) => other !== choice).map((into) => ({ label: displayName(into), action: () => merge(choice, into) }));
    menu.open([], item, event, [
      ...(others.length > 0 ? [{ label: "Merge into…", action: () => menu.open([], item, event, others) }] : []),
      { label: folderTally(choice).trashed ? "Put back" : "Trash", action: () => toggleTrash(choice) },
      { label: "Remove", action: () => remove(choice) },
    ]);
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

  /** Makes `item` take the dragged files into the Suggested Folder `to`, or leave them where they are when `to` is null, and merge another dragged folder into `to`. */
  function dropTarget(item, to) {
    const holds = (path) => (to ? to.files.has(path) : state.left.has(path));
    const accepts = () => (view.draggedFolder ? to !== null && to !== view.draggedFolder : view.dragged !== null && view.dragged.some((path) => !holds(path)));
    item.addEventListener("dragover", (event) => {
      if (!accepts()) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
      item.classList.add("drop-target");
    });
    item.addEventListener("dragleave", (event) => {
      if (!item.contains(event.relatedTarget)) item.classList.remove("drop-target");
    });
    item.addEventListener("drop", (event) => {
      item.classList.remove("drop-target");
      if (!accepts()) return;
      event.preventDefault();
      if (!view.draggedFolder) return ctx.selection.move(view.dragged, to);
      // The folder's sidebar item goes with it, so its own drag may not get to end.
      const from = view.draggedFolder;
      view.draggedFolder = null;
      merge(from, to);
    });
  }

  return { node, draw, showProgress };
}
