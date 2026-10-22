// Suggested Folders: the step after the duplicate step, given a fresh scan of the Scan Root.
// It needs the Grouping Models: without them it offers them, or shows their download. With them, it
// reads the loose files (Cancel goes back with nothing changed), then shows the Suggested Folders as
// a Plan: one card per folder, with its name, where it goes, what to do with it and its files, then
// the files left where they are, by kind and newest first. A file can be dragged into another folder,
// or out to the files left where they are. The Grouping settings group the files again, with no model
// run, and are remembered. Approving runs the Plan as its own Session.
import {
  analyseLooseFiles,
  buildSuggestedFoldersPlan,
  cancelAnalysis,
  describeError,
  groupingModelsStatus,
  pathSeparator,
  pickFolder,
  regroup,
  runPlan,
} from "../api.js";
import { el } from "../dom.js";
import { drawMore } from "../file-list.js";
import { count, formatDate, formatSize, plural, splitPath } from "../format.js";
import { icon } from "../icons.js";
import { groupingModelsPanel } from "../models-panel.js";
import { describeRunProgress, describeSummary } from "../outcomes.js";
import { busy, showStatus } from "../status.js";
import {
  byKindNewestFirst,
  DEFAULT_SETTINGS,
  groupingSettings,
  initialChoices,
  moveFile,
  nameMissing,
  planChoices,
  totals,
} from "../suggested-choices.js";

/** How many of a card's files are listed before "Show more". */
const FILES_SHOWN = 10;
const SETTINGS_KEY = "suggested-folders-settings";
/** Whether the Grouping settings are open, kept while they regroup the screen. */
let settingsOpen = false;

/** The file or folder's own name, from its full `path`. */
const fileName = (path) => splitPath(path, pathSeparator).name;

/** "Reading 12 of 85 files…" from the analysis's `{ finished, total }` progress, then the file it is on. */
const describeAnalysisProgress = ({ finished, total, current }) => `Reading ${count(finished)} of ${plural(total, "file")}…\n${current}`;

export async function render(root, nav, result) {
  const screen = el("div");
  root.append(screen);
  const rootName = splitPath(result.root, pathSeparator).name;
  const sizes = new Map(result.entries.map((entry) => [entry.path, entry.size]));
  const modified = new Map(result.entries.map((entry) => [entry.path, entry.modified]));
  let settings = loadSettings();
  const titled = (...rest) => titledAs("Suggested Folders", ...rest);
  const titledAs = (title, ...rest) => [el("h1", {}, title), el("p", { className: "path root-path" }, icon("folder"), result.root), ...rest];
  const homeButton = () => {
    const home = el("button", { type: "button", className: "primary" }, "Home");
    home.addEventListener("click", () => nav.home());
    return home;
  };
  const failed = (error) =>
    screen.replaceChildren(...titled(el("p", { className: "status error" }, describeError(error)), el("div", { className: "actions" }, homeButton())));

  function withoutModels() {
    const totalSize = result.entries.reduce((sum, entry) => sum + entry.size, 0);
    screen.replaceChildren(
      ...titled(
        el("p", { className: "lead" }, `Scanned again: ${plural(result.entries.length, "item")} · ${formatSize(totalSize)}`),
        groupingModelsPanel(analyse),
        el("p", { className: "note" }, "Sorting by kind, name and date, without the grouping models, is coming."),
        el("div", { className: "actions" }, homeButton()),
      ),
    );
  }

  async function analyse() {
    const status = el("p", { className: "status" }, "Reading the files…");
    const bar = el("progress", { className: "analysis-progress" });
    const cancel = el("button", { type: "button" }, "Cancel");
    let cancelled = false;
    // Goes back once the analysis has stopped, so it isn't still running when the user moves on.
    cancel.addEventListener("click", () => {
      cancelled = true;
      cancel.disabled = true;
      showStatus(status, "Stopping…");
      cancelAnalysis().catch((error) => showStatus(status, describeError(error), true));
    });
    screen.replaceChildren(...titled(status, bar, el("div", { className: "actions" }, cancel)));
    try {
      const { grouping, device } = await analyseLooseFiles(settings, (progress) => {
        if (cancelled) return;
        showStatus(status, describeAnalysisProgress(progress));
        bar.max = progress.total;
        bar.value = progress.finished;
      });
      if (cancelled) nav.home();
      else review(grouping, device);
    } catch (error) {
      if (cancelled) nav.home();
      else failed(error);
    }
  }

  function review(grouping, device) {
    const choices = initialChoices(grouping.folders);
    const summaryOf = (path) => grouping.summaries[path] ?? fileName(path);
    const kindOf = (path) => grouping.kinds[path];
    const modifiedOf = (path) => modified.get(path);
    const deviceNote = device ? el("p", { className: "note" }, `Files read on ${device}.`) : "";
    const settingsPanel = groupingSettingsPanel(device);

    if (choices.length === 0) {
      screen.replaceChildren(
        ...titledAs(
          "No folders to suggest",
          el("p", { className: "lead" }, "No files here are alike enough to share a folder, so everything stays where it is."),
          el("div", { className: "actions" }, homeButton()),
          settingsPanel,
          el("h2", {}, `Left where they are (${count(grouping.ungrouped.length)})`),
          ...leftLists(grouping.ungrouped, (path) => leftRow(path)),
          deviceNote,
        ),
      );
      return;
    }

    // Dragging a file: its path, while a drag is on.
    let dragged = null;
    const left = new Set(grouping.ungrouped);
    const leftSection = el("section", { className: "left-section" });
    const redraws = [];
    const move = (path, to) => {
      moveFile(choices, left, path, to);
      redraws.forEach((redraw) => redraw());
      drawLeft();
    };

    const heading = el("h1", {});
    const run = el("button", { type: "button", className: "primary" });
    const cancel = el("button", { type: "button" }, "Cancel");
    cancel.addEventListener("click", () => nav.home());
    const status = el("p", { className: "status" });
    const drawHeader = () => {
      const { moved, trashed } = totals(choices);
      const n = moved + trashed;
      heading.textContent = n ? `${plural(n, "file")} will move` : "Nothing will change";
      run.textContent = n ? `Move ${plural(n, "file")}` : "Nothing to move";
      run.disabled = n === 0 || choices.some(nameMissing);
    };

    run.addEventListener("click", async () => {
      const session = await busy(status, [run, cancel], "Planning…", async () => {
        const plan = await buildSuggestedFoldersPlan(planChoices(choices));
        const blocked = plan.operations.filter((op) => op.error);
        if (blocked.length > 0) {
          const { reason, error } = blocked[0];
          throw new Error(
            `${plural(blocked.length, "file")} can't be moved into “${reason.folder}”: ${error}. Choose another place for that folder, or skip it.`,
          );
        }
        const approved = plan.operations.filter((op) => op.ticked).map((op) => op.id);
        return runPlan(approved, (progress) => showStatus(status, describeRunProgress(progress)));
      });
      if (session) nav.session(session.sessionId, null, describeSummary(session, "moved"));
      else drawHeader();
    });

    screen.replaceChildren(
      el(
        "header",
        { className: "plan-head" },
        el(
          "div",
          {},
          heading,
          el("p", { className: "lead" }, "Nothing is deleted, and you can undo it all from History."),
        ),
        el("div", { className: "actions" }, cancel, run),
      ),
      status,
      settingsPanel,
      el("div", { className: "folder-cards" }, ...choices.map(folderCard)),
      leftSection,
      deviceNote,
    );
    drawHeader();
    drawLeft();
    dropTarget(leftSection, null);

    /** The files left where they are, each one draggable into a folder. */
    function drawLeft() {
      leftSection.replaceChildren(
        el("h2", {}, `Left where they are (${count(left.size)})`),
        ...(left.size ? leftLists([...left], (path) => draggable(leftRow(path), path)) : [el("p", { className: "note" }, "Drag a file here to leave it where it is.")]),
      );
    }

    /** Makes `row` drag the file at `path`. */
    function draggable(row, path) {
      row.draggable = true;
      row.addEventListener("dragstart", (event) => {
        dragged = path;
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", path);
        row.classList.add("dragging");
      });
      row.addEventListener("dragend", () => {
        dragged = null;
        row.classList.remove("dragging");
      });
      return row;
    }

    /** Makes `node` take a dragged file into the Suggested Folder `to`, or leave it where it is when `to` is null. */
    function dropTarget(node, to) {
      const holds = (path) => (to ? to.files.has(path) : left.has(path));
      const accepts = () => dragged !== null && !holds(dragged);
      node.addEventListener("dragover", (event) => {
        if (!accepts()) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
        node.classList.add("drop-target");
      });
      node.addEventListener("dragleave", (event) => {
        if (!node.contains(event.relatedTarget)) node.classList.remove("drop-target");
      });
      node.addEventListener("drop", (event) => {
        node.classList.remove("drop-target");
        if (!accepts()) return;
        event.preventDefault();
        move(dragged, to);
      });
    }

    /** The card for one Suggested Folder. */
    function folderCard(choice) {
      const name = el("input", { type: "text", className: "folder-name", value: choice.name, ariaLabel: "Folder name", spellcheck: false });
      const nameError = el("p", { className: "status error folder-name-error" });
      const size = el("span", { className: "muted" });
      const where = el("p", { className: "folder-where" });
      const existingName = choice.folder.existing ? fileName(choice.folder.existing) : null;
      const pick = el(
        "select",
        { ariaLabel: "What to do with this folder" },
        existingName ? el("option", { value: "addTo" }, `Add to ${existingName}`) : "",
        el("option", { value: "createHere" }, existingName ? "New folder here" : "Create here"),
        el("option", { value: "createIn" }, "Create elsewhere…"),
        el("option", { value: "trashAll" }, "Trash all"),
        el("option", { value: "skip" }, "Skip"),
      );
      pick.value = choice.choice;
      const list = el("ul", { className: "folder-files", ariaLabel: "Files" });
      const more = el("div", { className: "plan-more" });
      let limit = FILES_SHOWN;

      const pickElsewhere = async () => {
        try {
          const folder = await pickFolder("Pick where to create the folder");
          if (folder) {
            choice.elsewhere = folder;
            choice.choice = "createIn";
          }
        } catch (error) {
          showStatus(status, describeError(error), true);
        }
        pick.value = choice.choice;
        draw();
      };
      pick.addEventListener("change", () => {
        if (pick.value === "createIn") return pickElsewhere();
        choice.choice = pick.value;
        draw();
      });
      name.addEventListener("input", () => {
        choice.name = name.value;
        drawName();
        drawHeader();
      });

      const drawName = () => {
        const missing = nameMissing(choice);
        showStatus(nameError, missing ? "A folder needs a name." : "", missing);
        name.ariaInvalid = String(missing);
      };

      function drawWhere() {
        const change = el("button", { type: "button", className: "link" }, "Change…");
        change.addEventListener("click", pickElsewhere);
        const parts = {
          addTo: () => ["Add to ", el("strong", {}, existingName)],
          createHere: () => [`New folder in ${rootName}`],
          createIn: () => ["New folder in ", el("span", { className: "path" }, choice.elsewhere), " ", change],
          trashAll: () => ["Every file is trashed"],
          skip: () => ["Every file stays where it is"],
        };
        where.replaceChildren(...parts[choice.choice]());
      }

      function fileRow(path) {
        const state = choice.files.get(path);
        const set = (next) => () => {
          choice.files.set(path, next);
          draw();
        };
        const button = (label, next) => {
          const node = el("button", { type: "button", className: "link", disabled: choice.choice === "skip" }, label);
          node.addEventListener("click", set(next));
          return node;
        };
        const note = { trash: " · Trashed", takeOut: " · Stays where it is" }[state] ?? "";
        const actions = state === "inFolder" ? [button("Trash", "trash"), button("Take out", "takeOut")] : [button("Put back", "inFolder")];
        const row = el(
          "li",
          { className: "folder-file", title: "Drag to another folder" },
          el("span", { className: "folder-file-name", title: summaryOf(path) }, fileName(path), el("span", { className: "muted" }, note)),
          el("span", { className: "folder-file-actions" }, ...actions),
        );
        row.classList.toggle("out", state !== "inFolder");
        return draggable(row, path);
      }

      function draw() {
        drawName();
        drawWhere();
        const paths = [...choice.files.keys()];
        size.textContent = ` · ${plural(paths.length, "file")} · ${formatSize(paths.reduce((sum, path) => sum + (sizes.get(path) ?? 0), 0))}`;
        const shown = Math.min(paths.length, limit);
        list.classList.toggle("skipped", choice.choice === "skip");
        list.replaceChildren(...paths.slice(0, shown).map(fileRow));
        drawMore(more, paths.length, shown, (next) => {
          limit = next;
          draw();
        });
        drawHeader();
      }

      const node = el(
        "section",
        { className: "plan-card folder-card" },
        el("div", { className: "folder-card-head" }, icon("folder"), name, size),
        nameError,
        el("div", { className: "folder-card-choice" }, where, pick),
        list,
        more,
      );
      draw();
      redraws.push(draw);
      dropTarget(node, choice);
      return node;
    }

    /** A file left where it is: its name, with its summary on hover, and when it was last changed. */
    function leftRow(path) {
      return el(
        "li",
        { className: "folder-file" },
        el("span", { className: "folder-file-name", title: summaryOf(path) }, fileName(path)),
        el("span", { className: "muted folder-file-date" }, formatDate(modifiedOf(path))),
      );
    }

    /** `paths` under a heading per kind, each kind newest first, with `rowFor(path)` per file. */
    function leftLists(paths, rowFor) {
      return byKindNewestFirst(paths, kindOf, modifiedOf).flatMap(([kind, files]) => [
        el("h3", { className: "left-kind" }, `${kind} (${count(files.length)})`),
        el("ul", { className: "folder-files left-files" }, ...files.map(rowFor)),
      ]);
    }
  }

  /**
   * The Grouping settings: how alike files must be to share a folder, and a folder to go into an existing
   * one. Changing one groups the files again from what the models said, starting the review over.
   */
  function groupingSettingsPanel(device) {
    const status = el("p", { className: "status" });
    const apply = async (next) => {
      settings = next;
      saveSettings(settings);
      try {
        review(await regroup(settings), device);
      } catch (error) {
        showStatus(status, describeError(error), true);
      }
    };
    const slider = (label, key) => {
      const value = el("output", {}, settings[key].toFixed(2));
      const input = el("input", { type: "range", min: 0.5, max: 0.99, step: 0.01, value: settings[key], ariaLabel: label });
      input.addEventListener("input", () => (value.textContent = Number(input.value).toFixed(2)));
      input.addEventListener("change", () => apply({ ...settings, [key]: Number(input.value) }));
      return el("label", { className: "grouping-setting" }, el("span", {}, label), input, value);
    };
    const reset = el("button", { type: "button", className: "link" }, "Reset");
    reset.addEventListener("click", () => apply({ ...DEFAULT_SETTINGS }));
    const panel = el(
      "details",
      { className: "grouping-settings", open: settingsOpen },
      el("summary", {}, "Grouping"),
      el("p", { className: "note" }, "Lower groups more files together; higher keeps folders tighter. Changing these starts your edits over."),
      slider("Files share a folder at", "folderThreshold"),
      slider("A folder goes into an existing one at", "existingFolderThreshold"),
      reset,
      status,
    );
    panel.addEventListener("toggle", () => (settingsOpen = panel.open));
    return panel;
  }

  try {
    const { state } = await groupingModelsStatus();
    if (state === "installed") analyse();
    else withoutModels();
  } catch (error) {
    failed(error);
  }
}

/** The Grouping settings last used, or the defaults. */
function loadSettings() {
  try {
    return groupingSettings(JSON.parse(localStorage.getItem(SETTINGS_KEY)));
  } catch {
    // Nothing saved, or storage is unavailable: use the defaults.
    return { ...DEFAULT_SETTINGS };
  }
}

function saveSettings(settings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  } catch {
    // Storage is unavailable: the settings last until the screen closes.
  }
}
