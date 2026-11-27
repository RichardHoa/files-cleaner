// Rows have no buttons on purpose: every action on files comes from the toolbar or a menu.
import { describeError, onAnotherDisk, pickFolder } from "../../shared/api.js";
import { drawMore, el, icon, showStatus } from "../../shared/dom.js";
import { count, formatDate, formatSize, plural } from "../../shared/format.js";
import { showMenu } from "../../shared/menus.js";
import { describeRule } from "../rules/rules.js";
import { byKindNewestFirst, nameMissing, otherKeptCopies } from "./review-choices.js";
import { fileName, FILES_SHOWN, trashedMark, UNSORTED } from "./selection.js";

function basisText(folder, existingName) {
  const fits = existingName ? ` · fits your folder “${existingName}”` : "";
  if (folder.basis === "topic") return `Topic: most files are about “${folder.name}”${fits}`;
  if (folder.basis === "kind") return `File type: not read inside${fits}`;
  if (folder.basis === "existing") return `Fits your folder “${existingName}”`;
  if (folder.basis === "rule") return `Rule: ${folder.rule.words ? `“${folder.rule.words}”` : describeRule(folder.rule)}${fits}`;
  return "";
}

export const foldersNote = (rootName) =>
  `Folders for the loose files in ${rootName}. Each file is moved into the folder shown; nothing is trashed unless you mark it.`;

export function folderPanes(ctx) {
  const { state } = ctx;
  const panels = new WeakMap();

  const summaryOf = (path) => state.summaryOf(path) ?? fileName(path);
  const fileText = (path) => {
    const summary = state.summaryOf(path);
    return el(
      "div",
      { className: "folder-file-text" },
      el("span", { className: "folder-file-name", title: summaryOf(path) }, fileName(path)),
      ...(summary ? [el("span", { className: "folder-file-summary muted" }, summary)] : []),
    );
  };

  const panel = (choice) => {
    if (!panels.has(choice)) panels.set(choice, folderPanel(choice));
    return panels.get(choice);
  };

  /** A Kept Copy that isn't filed must stay where it is, so it can only be opened or shown. */
  function leftPanel() {
    const others = otherKeptCopies(state.duplicates);
    const row = (path) => {
      if (!others.has(path)) return ctx.selection.selectable(leftRow(path), path);
      const node = leftRow(path);
      node.tabIndex = 0;
      return ctx.selection.copyMenu(node, path);
    };
    const head = el("h2", {}, `${UNSORTED} (${count(state.left.size)})`);
    const stay = el("p", { className: "unsorted-note muted" }, "Loose files no folder took. They stay where they are.");
    if (state.choices.length > 0) return [head, stay, ...leftLists([...state.left], row)];
    return [
      head,
      stay,
      el("p", { className: "note" }, state.grouping.folders.length > 0 ? "No folders are left." : "No files here are alike enough to share a folder."),
      ...leftLists([...state.left], row),
    ];
  }

  function folderPanel(choice) {
    const name = el("input", { type: "text", className: "folder-name", value: choice.name, ariaLabel: "Folder name", spellcheck: false });
    const nameError = el("p", { className: "status error folder-name-error" });
    const size = el("span", { className: "muted" });
    const existingName = choice.folder.existing ? fileName(choice.folder.existing) : null;
    const basis = el("p", { className: "folder-basis muted" });
    const location = el("select", { ariaLabel: "Location" });
    const locationError = el("p", { className: "status error folder-location-error" });
    const list = el("ul", { className: "folder-files", ariaLabel: "Files" });
    const empty = el("p", { className: "note" }, "Empty");
    const more = el("div", { className: "plan-more" });
    const actions = el("button", { type: "button", className: "folder-actions", ariaLabel: "Folder actions", title: "Folder actions", ariaHasPopup: "menu" }, icon("more"));
    actions.addEventListener("click", () => showMenu(ctx.sidebar.folderMenuItems(choice)));
    let limit = FILES_SHOWN;

    /** Another disk is refused: a move there would be a copy and a delete. */
    const pickElsewhere = async () => {
      showStatus(locationError, "");
      try {
        const folder = await pickFolder("Pick where to create the folder");
        if (folder && (await onAnotherDisk([...choice.files.keys()], folder))) {
          showStatus(locationError, `“${fileName(folder)}” is on another disk. Pick a folder on the same disk.`, true);
        } else if (folder) state.setChoice(choice, "createIn", folder);
      } catch (error) {
        showStatus(locationError, describeError(error), true);
      }
      draw();
    };
    location.addEventListener("change", () => {
      if (location.value === "other") return pickElsewhere();
      state.setChoice(choice, location.value);
      showStatus(locationError, "");
      draw();
    });
    name.addEventListener("input", () => {
      state.rename(choice, name.value);
      drawName();
      ctx.redraw.all();
    });

    /** Also called when the sidebar renames it. */
    const drawName = () => {
      if (name.value !== choice.name) name.value = choice.name;
      const missing = nameMissing(choice);
      showStatus(nameError, missing ? "A folder needs a name." : "", missing);
      name.ariaInvalid = String(missing);
    };

    function drawLocation() {
      const options = [
        ...(existingName ? [el("option", { value: "addTo", title: choice.folder.existing }, existingName)] : []),
        el("option", { value: "createHere" }, `New folder in ${ctx.rootName}`),
        ...(choice.elsewhere ? [el("option", { value: "createIn", title: choice.elsewhere }, `New folder in ${fileName(choice.elsewhere)}`)] : []),
        el("option", { value: "other" }, "Other…"),
      ];
      location.replaceChildren(...options);
      location.value = choice.choice;
    }

    function fileRow(path) {
      const trashed = choice.files.get(path) === "trash";
      const row = el("li", { className: "folder-file" }, fileText(path), trashed ? trashedMark() : "");
      row.classList.toggle("out", trashed);
      return ctx.selection.selectable(row, path);
    }

    function draw() {
      // Once another folder is merged into it, it has no basis left.
      basis.textContent = basisText(choice.folder, existingName);
      basis.hidden = !basis.textContent;
      drawName();
      drawLocation();
      const paths = [...choice.files.keys()];
      size.textContent = ` · ${plural(paths.length, "file")} · ${formatSize(paths.reduce((sum, path) => sum + (state.sizeOf(path) ?? 0), 0))}`;
      const shown = Math.min(paths.length, limit);
      list.replaceChildren(...paths.slice(0, shown).map(fileRow));
      empty.hidden = paths.length > 0;
      drawMore(more, paths.length, shown, (next) => {
        limit = next;
        draw();
      });
      ctx.redraw.all();
    }

    const node = el(
      "div",
      { className: "folder-panel" },
      el("div", { className: "folder-panel-head" }, icon("folder"), name, size, actions),
      nameError,
      el("p", { className: "pane-note muted" }, foldersNote(ctx.rootName)),
      basis,
      el("label", { className: "folder-location" }, el("span", { className: "muted" }, "Location"), location),
      locationError,
      list,
      empty,
      more,
    );
    return { node, draw, drawName };
  }

  function leftRow(path) {
    const trashed = state.left.trashed.has(path);
    const row = el(
      "li",
      { className: "folder-file" },
      fileText(path),
      el("span", { className: "folder-file-end" }, trashed ? trashedMark() : "", el("span", { className: "muted folder-file-date" }, formatDate(state.modifiedOf(path)))),
    );
    row.classList.toggle("out", trashed);
    return row;
  }

  function leftLists(paths, rowFor) {
    return byKindNewestFirst(paths, (path) => state.kindOf(path), (path) => state.modifiedOf(path)).flatMap(([kind, files]) => [
      el("h3", { className: "left-kind" }, `${kind} (${count(files.length)})`),
      el("ul", { className: "folder-files left-files" }, ...files.map(rowFor)),
    ]);
  }

  return { panel, drawName: (choice) => panel(choice).drawName(), leftPanel };
}
