// The Review screen's folder panes: a Suggested Folder, with its name, why it was suggested and its one
// Location popup, and the files left where they are, shown as Unsorted. Rows have no buttons: a trashed
// file is dimmed with a trash icon, and every action comes from the selection.
import { describeError, onAnotherDisk, pickFolder } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { drawMore } from "../../shared/plan/file-list.js";
import { count, formatDate, formatSize, plural } from "../../shared/format.js";
import { icon } from "../../shared/icons.js";
import { showStatus } from "../../shared/status.js";
import { describeRule } from "../rules/rule-text.js";
import { byKindNewestFirst, nameMissing, otherKeptCopies } from "./review-choices.js";
import { FILES_SHOWN, fileName, trashedMark, UNSORTED } from "./pane-helpers.js";

/** Why the tool suggested `folder`, in a few words, given the name of the existing folder it matches, if any; "" for a folder the user added. */
function basisText(folder, existingName) {
  const fits = existingName ? ` · fits your folder “${existingName}”` : "";
  if (folder.basis === "topic") return `Topic: most files are about “${folder.name}”${fits}`;
  if (folder.basis === "kind") return `File type: not read inside${fits}`;
  if (folder.basis === "existing") return `Fits your folder “${existingName}”`;
  if (folder.basis === "rule") return `Rule: ${folder.rule.words ? `“${folder.rule.words}”` : describeRule(folder.rule)}${fits}`;
  return "";
}

/**
 * The folder panes of the Review screen with context `ctx`: `panel(choice)`, a Suggested Folder's, made the
 * first time it's needed, `drawName(choice)` and `leftPanel()`.
 */
export function folderPanes(ctx) {
  const { state } = ctx;
  const panels = new WeakMap();

  const summaryOf = (path) => state.summaryOf(path) ?? fileName(path);
  /** The file at `path`'s name, then its one-sentence summary below it if it was read. */
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

  /** The files left where they are, as Unsorted, by kind. A Kept Copy that isn't filed stays where it is, so it can only be opened or shown. */
  function leftPanel() {
    const others = otherKeptCopies(state.duplicates);
    const row = (path) => {
      if (!others.has(path)) return ctx.selection.selectable(leftRow(path), path);
      const node = leftRow(path);
      node.tabIndex = 0;
      return ctx.selection.copyMenu(node, path);
    };
    const head = el("h2", {}, `${UNSORTED} (${count(state.left.size)})`);
    if (state.choices.length > 0) return [head, ...leftLists([...state.left], row)];
    return [
      head,
      el("p", { className: "note" }, state.grouping.folders.length > 0 ? "No folders are left." : "No files here are alike enough to share a folder."),
      ...leftLists([...state.left], row),
    ];
  }

  /** The pane for one Suggested Folder: its name, why it was suggested, where it goes, and its files. */
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
    let limit = FILES_SHOWN;

    /** Asks for a folder on the files' disk to make the folder in; one on another disk is refused, beside the popup. */
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

    /** The name and whether it's missing, also when it was renamed in the sidebar. */
    const drawName = () => {
      if (name.value !== choice.name) name.value = choice.name;
      const missing = nameMissing(choice);
      showStatus(nameError, missing ? "A folder needs a name." : "", missing);
      name.ariaInvalid = String(missing);
    };

    /** The Location popup: the existing folder it matched, if any, the Scan Root, the folder picked with Other… before, if any, and Other…. */
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
      el("div", { className: "folder-panel-head" }, icon("folder"), name, size),
      nameError,
      basis,
      el("label", { className: "folder-location" }, el("span", { className: "muted" }, "Location"), location),
      locationError,
      list,
      empty,
      more,
    );
    return { node, draw, drawName };
  }

  /** A file left where it is: its name, with its summary below, when it was last changed, and the trash icon if it's trashed. */
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

  /** `paths` under a heading per kind, each kind newest first, with `rowFor(path)` per file. */
  function leftLists(paths, rowFor) {
    return byKindNewestFirst(paths, (path) => state.kindOf(path), (path) => state.modifiedOf(path)).flatMap(([kind, files]) => [
      el("h3", { className: "left-kind" }, `${kind} (${count(files.length)})`),
      el("ul", { className: "folder-files left-files" }, ...files.map(rowFor)),
    ]);
  }

  return { panel, drawName: (choice) => panel(choice).drawName(), leftPanel };
}
