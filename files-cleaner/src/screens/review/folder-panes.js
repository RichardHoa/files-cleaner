// The Review screen's folder panes: a Suggested Folder, the files left where they are and the Emptied
// Folders, with their rows. The Emptied Folders depend on everything the Plan moves, so they are worked out
// by the backend again a moment after each change.
import { describeError, pathSeparator, pickFolder, previewReviewPlan } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { drawMore } from "../../shared/plan/file-list.js";
import { count, formatDate, formatSize, plural, trashPlace } from "../../shared/format.js";
import { icon } from "../../shared/icons.js";
import { tickState, toggleAll } from "../../shared/plan/plan-selection.js";
import { showStatus } from "../../shared/status.js";
import { byKindNewestFirst, LeftFiles, leftPartChoices, nameMissing, otherKeptCopies, partChoices, planChoices, totals } from "./review-choices.js";
import { FILES_SHOWN, fileName } from "./pane-helpers.js";

/** How long the Emptied Folders wait after a change before they're worked out again, in ms. */
const EMPTIED_DELAY = 300;

/** Why the tool suggested `folder`, in a few words, given the name of the existing folder it matches, if any; "" for a folder the user added. */
function basisText(folder, existingName) {
  const fits = existingName ? ` · fits your folder “${existingName}”` : "";
  if (folder.basis === "topic") return `Topic: most files are about “${folder.name}”${fits}`;
  if (folder.basis === "kind") return `File type: not read inside${fits}`;
  if (folder.basis === "existing") return `Fits your folder “${existingName}”`;
  return "";
}

/**
 * The folder panes of the Review screen with context `ctx`: `panel(choice)`, a Suggested Folder's, made the
 * first time it's needed, `drawName(choice)`, `leftPanel()` and `emptiedPanel()`, and `refreshEmptied()`
 * working out the Emptied Folders again a moment later, unless `holdEmptied()` comes first.
 */
export function folderPanes(ctx) {
  const { state, view, status, pane } = ctx;
  const panels = new WeakMap();

  const summaryOf = (path) => state.summaryOf(path) ?? fileName(path);
  /** The file at `path`'s name, then its one-sentence summary below it if it was read. */
  const fileText = (path, name) => {
    const summary = state.summaryOf(path);
    return el("div", { className: "folder-file-text" }, name, ...(summary ? [el("span", { className: "folder-file-summary muted" }, summary)] : []));
  };

  const panel = (choice) => {
    if (!panels.has(choice)) panels.set(choice, folderPanel(choice));
    return panels.get(choice);
  };

  // Only the latest answer counts.
  let emptiedTimer = 0;
  let emptiedAsked = 0;
  const holdEmptied = () => clearTimeout(emptiedTimer);
  const refreshEmptied = () => {
    clearTimeout(emptiedTimer);
    emptiedTimer = setTimeout(async () => {
      if (!pane.isConnected) return;
      const asked = ++emptiedAsked;
      try {
        const plan = await previewReviewPlan([], planChoices(state.choices, state.left, state.ran));
        if (asked !== emptiedAsked || !pane.isConnected) return;
        state.setEmptied(plan.operations.filter((op) => op.reason.type === "emptiedFolder"));
        if (view.current === "emptied") ctx.redraw.pane();
        else {
          ctx.redraw.sidebar();
          ctx.redraw.header();
        }
      } catch {
        // A name that can't be a folder name: the Emptied Folders stay as they were until it's fixed.
      }
    }, EMPTIED_DELAY);
  };

  /**
   * The files left where they are, by kind, for the pane, with a button trashing those marked Trash on their
   * own, and what that did last. A Kept Copy that isn't filed stays where it is, and a file already run is
   * gone, so either can only be opened or shown.
   */
  function leftPanel() {
    const others = otherKeptCopies(state.duplicates);
    const row = (path) => {
      if (others.has(path) || state.ran.has(path)) {
        const node = leftRow(path);
        node.tabIndex = 0;
        return ctx.selection.copyMenu(node, path);
      }
      return ctx.selection.selectable(leftRow(path, [trashToggle(path)]), path);
    };
    const trashing = [...state.left.trashed].filter((path) => !state.ran.has(path));
    const trash = el("button", { type: "button", disabled: view.running || trashing.length === 0 }, trashing.length ? `Trash just these ${plural(trashing.length, "file")}` : "Nothing to trash");
    trash.addEventListener("click", () => ctx.sessions.trashPart("left", leftPartChoices(state.left, state.ran), trash));
    const head = [
      el("div", { className: "duplicates-head" }, el("h2", {}, `Left where they are (${count(state.left.size)})`), trash),
      ...ctx.sessions.trashOutcomes("left"),
    ];
    if (state.choices.length === 0) {
      return [
        ...head,
        el(
          "p",
          { className: "note" },
          state.grouping.folders.length > 0 ? "No folders are left." : "No files here are alike enough to share a folder.",
        ),
        ...leftLists([...state.left], row),
      ];
    }
    return [...head, ...leftLists([...state.left], row)];
  }

  /** The folders the Plan leaves empty, each unticked until the user ticks it. */
  function emptiedPanel() {
    const ids = state.emptied.filter((op) => !op.error).map((op) => op.id);
    const all = el("input", { type: "checkbox", disabled: ids.length === 0 });
    const boxes = [];
    const drawTicks = () => {
      const ticks = tickState(ids, state.emptiedTicked);
      all.checked = ticks === "all";
      all.indeterminate = ticks === "some";
      for (const [op, box] of boxes) box.checked = state.emptiedTicked.has(op.id);
      ctx.redraw.header();
    };
    all.addEventListener("change", () => {
      state.tickEmptied(toggleAll(ids, state.emptiedTicked));
      drawTicks();
    });
    const rows = state.emptied.map((op) => {
      const folder = op.relativePath.join("/");
      const box = el("input", { type: "checkbox", disabled: Boolean(op.error), ariaLabel: `Move the empty folder ${folder}` });
      box.addEventListener("change", () => {
        const ticked = new Set(state.emptiedTicked);
        if (box.checked) ticked.add(op.id);
        else ticked.delete(op.id);
        state.tickEmptied(ticked);
        drawTicks();
      });
      boxes.push([op, box]);
      return el(
        "li",
        { className: "folder-file" },
        el("label", { className: "emptied-folder" }, box, el("span", { className: "path" }, `${folder}/`)),
        op.error ? el("span", { className: "status error" }, "Can't move") : "",
      );
    });
    drawTicks();
    return [
      el("h2", { title: "Folders left empty once their files move. Left out unless you tick them; only moved if still empty." }, "Emptied Folders"),
      el("label", { className: "plan-include" }, all, "Include all"),
      state.emptied.length ? el("ul", { className: "folder-files" }, ...rows) : el("p", { className: "note" }, "No folder is left empty."),
    ];
  }

  /** The pane for one Suggested Folder: its name, where it goes, what to do with it, and its files. */
  function folderPanel(choice) {
    const name = el("input", { type: "text", className: "folder-name", value: choice.name, ariaLabel: "Folder name", spellcheck: false });
    const nameError = el("p", { className: "status error folder-name-error" });
    const size = el("span", { className: "muted" });
    const where = el("p", { className: "folder-where" });
    const existingName = choice.folder.existing ? fileName(choice.folder.existing) : null;
    const basis = el("p", { className: "folder-basis muted" });
    const options = [
      ...(existingName ? [["addTo", `Add to ${existingName}`]] : []),
      ["createHere", existingName ? "New folder here" : "Create here"],
      ["createIn", "Elsewhere…"],
      ["trashAll", "Trash all"],
    ];
    const segments = options.map(([value, label]) => {
      const segment = el("button", { type: "button", role: "radio", className: "segment" }, label);
      segment.addEventListener("click", () => {
        // "Elsewhere…" always asks where, so it also changes the folder picked before.
        if (value === "createIn") return pickElsewhere();
        state.setChoice(choice, value);
        draw();
      });
      return [value, segment];
    });
    const pick = el("div", { className: "segmented", role: "radiogroup", ariaLabel: "What to do with this folder" }, ...segments.map(([, segment]) => segment));
    // Once its moves ran, the folder is finished: its name and where it goes stay as they were.
    const drawPick = () => {
      for (const [value, segment] of segments) {
        segment.ariaChecked = String(value === choice.choice);
        segment.disabled = Boolean(choice.moved);
      }
    };
    const runs = el("div", { className: "folder-panel-run" });
    const outcomes = el("div");
    const list = el("ul", { className: "folder-files", ariaLabel: "Files" });
    const empty = el("p", { className: "note" }, "Empty");
    const more = el("div", { className: "plan-more" });
    let limit = FILES_SHOWN;

    const pickElsewhere = async () => {
      try {
        const folder = await pickFolder("Pick where to create the folder");
        if (folder) state.setChoice(choice, "createIn", folder);
      } catch (error) {
        showStatus(status, describeError(error), true);
      }
      draw();
    };
    name.addEventListener("input", () => {
      state.rename(choice, name.value);
      drawName();
      ctx.redraw.all();
    });

    /** The name and whether it's missing, also when it was renamed in the sidebar, with the buttons that need one. */
    const drawName = () => {
      if (name.value !== choice.name) name.value = choice.name;
      name.readOnly = Boolean(choice.moved);
      const missing = nameMissing(choice);
      showStatus(nameError, missing ? "A folder needs a name." : "", missing);
      name.ariaInvalid = String(missing);
      drawRuns();
    };

    function drawWhere() {
      const change = el("button", { type: "button", className: "link" }, "Change…");
      change.addEventListener("click", pickElsewhere);
      const parts = {
        addTo: () => ["Add to ", el("strong", {}, existingName)],
        createHere: () => [`New folder in ${ctx.rootName}`],
        createIn: () => ["New folder in ", el("span", { className: "path" }, choice.elsewhere), ...(choice.moved ? [] : [" ", change])],
        trashAll: () => [`Every file goes to ${trashPlace(pathSeparator)}`],
      };
      where.replaceChildren(...parts[choice.choice]());
    }

    function fileRow(path) {
      const file = choice.files.get(path);
      const set = (next) => () => {
        state.setFile(choice, path, next);
        draw();
      };
      const button = (label, next) => {
        const node = el("button", { type: "button", className: "small" }, label);
        node.addEventListener("click", set(next));
        return node;
      };
      const done = state.ran.has(path);
      const notes = { trash: " · Trashed", takeOut: " · Stays where it is", inFolder: choice.moved ? " · Not moved: retry it from its Session" : "" };
      const note = done ? (file === "trash" || choice.choice === "trashAll" ? " · Trashed" : " · Moved") : notes[file];
      // A file already run can only be opened; once the folder's moves ran, a file can't be put back into it.
      const actions = done
        ? []
        : file === "inFolder"
          ? [button("Trash", "trash"), button("Take out", "takeOut")]
          : choice.moved
            ? [file === "trash" ? button("Take out", "takeOut") : button("Trash", "trash")]
            : [button("Put back", "inFolder")];
      const row = el(
        "li",
        { className: "folder-file" },
        fileText(path, el("span", { className: "folder-file-name", title: summaryOf(path) }, fileName(path), el("span", { className: "muted" }, note))),
        el("span", { className: "folder-file-actions" }, ...actions),
      );
      row.classList.toggle("out", done || file !== "inFolder");
      return ctx.selection.selectable(row, path, { movable: !done });
    }

    /** The buttons moving the files going into the folder, and trashing those it trashes, each on its own, then what each did last. */
    function drawRuns() {
      const moving = choice.moved ? 0 : totals([choice], new LeftFiles(), pathSeparator, [], state.ran).moved;
      const trashing = partChoices(choice, "trash", state.ran)[0]?.files.length ?? 0;
      const buttons = [];
      if (choice.choice !== "trashAll" && !choice.moved) {
        const move = el("button", { type: "button", disabled: view.running || moving === 0 || nameMissing(choice) }, moving ? `Move just these ${plural(moving, "file")}` : "Nothing to move");
        move.addEventListener("click", () => ctx.sessions.moveFolder(choice, move));
        buttons.push(move);
      }
      if (trashing > 0) {
        const trash = el("button", { type: "button", disabled: view.running || nameMissing(choice) }, `Trash just these ${plural(trashing, "file")}`);
        trash.addEventListener("click", () => ctx.sessions.trashPart(choice, partChoices(choice, "trash", state.ran), trash));
        buttons.push(trash);
      }
      runs.replaceChildren(...buttons);
      const moved = choice.moved ? ctx.sessions.outcome(choice.moved, "moved") : [];
      outcomes.replaceChildren(...moved, ...ctx.sessions.trashOutcomes(choice));
    }

    function draw() {
      // Once another folder is merged into it, it has no basis left.
      basis.textContent = basisText(choice.folder, existingName);
      basis.hidden = !basis.textContent;
      drawName();
      drawPick();
      drawWhere();
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
      basis,
      nameError,
      el("div", { className: "folder-panel-choice" }, pick, where),
      runs,
      outcomes,
      list,
      empty,
      more,
    );
    return { node, draw, drawName };
  }

  /** A button trashing the file at `path`, left where it is, or putting it back once it's trashed. */
  function trashToggle(path) {
    const trashed = state.left.trashed.has(path);
    const button = el("button", { type: "button", className: "small" }, trashed ? "Put back" : "Trash");
    button.addEventListener("click", () => {
      state.trashLeft(path, !trashed);
      ctx.redraw.pane();
      // The file's row is drawn again: the focus goes with it.
      pane.querySelector(`[data-path="${CSS.escape(path)}"] button`)?.focus();
    });
    return button;
  }

  /** A file left where it is: its name, with its summary below, when it was last changed, and `actions`. */
  function leftRow(path, actions = []) {
    const trashed = state.left.trashed.has(path);
    const row = el(
      "li",
      { className: "folder-file" },
      fileText(path, el("span", { className: "folder-file-name", title: summaryOf(path) }, fileName(path), el("span", { className: "muted" }, trashed || state.ran.has(path) ? " · Trashed" : ""))),
      el("span", { className: "folder-file-actions" }, el("span", { className: "muted folder-file-date" }, formatDate(state.modifiedOf(path))), ...actions),
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

  return { panel, drawName: (choice) => panel(choice).drawName(), leftPanel, emptiedPanel, refreshEmptied, holdEmptied };
}
