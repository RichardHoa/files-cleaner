// Suggested Folders: the step after the duplicate step, given a fresh scan of the Scan Root.
// It needs the Grouping Models: without them it offers them, or shows their download. With them, it
// reads the loose files (Cancel goes back with nothing changed), then shows the Suggested Folders as
// a Plan, laid out like Mail: a sidebar lists the files left where they are, then each folder, and
// the pane beside it lists the one picked, with its name, where it goes, what to do with it and its
// files. Files are selected as in Finder, then moved by dragging them onto a folder in the sidebar,
// from their right-click menu, or with "Move to…". Files are grouped by the grouping settings, which are
// changed in Settings and apply the next time files are grouped. Approving runs the Plan as its own Session.
import {
  analyseLooseFiles,
  answerNotifications,
  buildSuggestedFoldersPlan,
  cancelAnalysis,
  describeError,
  groupingModelsStatus,
  notifications,
  notifyInBackground,
  pathSeparator,
  pickFolder,
  readingPace,
  runPlan,
  saveReadingPace,
} from "../api.js";
import { el } from "../dom.js";
import { drawMore, revealMenu } from "../file-list.js";
import { count, formatDate, formatSize, plural, splitPath } from "../format.js";
import { icon } from "../icons.js";
import { groupingModelsPanel } from "../models-panel.js";
import { describeRunProgress, describeSummary } from "../outcomes.js";
import { describeEstimate, readingEstimate, updatedAverage } from "../reading-estimate.js";
import { clickSelection } from "../row-selection.js";
import { busy, showStatus } from "../status.js";
import { byKindNewestFirst, initialChoices, moveFile, nameMissing, planChoices, totals } from "../suggested-choices.js";

/** How many of a card's files are listed before "Show more". */
const FILES_SHOWN = 50;
/** What the reading screen says about the wait, with notifications on, and off. */
const WAIT_TEXT = "Reading takes a while. You can switch to other apps; Files Cleaner will notify you when the folders are ready.";
const WAIT_TEXT_QUIET = "Reading takes a while. You can switch to other apps in the meantime.";
/** What the reading estimate calls this computer. */
const COMPUTER = /Mac/.test(navigator.userAgent) ? "Mac" : "computer";

/** The file or folder's own name, from its full `path`. */
const fileName = (path) => splitPath(path, pathSeparator).name;

/**
 * The analysis's `{ finished, total, current, phase }` progress as "Reading 12 of 85 files…", or what comes
 * before and after, then the file it is on, or that the models are loading.
 */
const describeAnalysisProgress = ({ finished, total, current, phase }) =>
  ({
    checking: () => `Checking which files were read before: ${count(finished)} of ${count(total)}…\n${current}`,
    loading: () => `Reading 0 of ${plural(total, "file")}…\nLoading the grouping models…`,
    reading: () => `Reading ${count(finished)} of ${plural(total, "file")}…\n${current}`,
    placing: () => `Finding folders for files left out: ${count(finished)} of ${count(total)}…\n${current}`,
  })[phase]();

/**
 * The line saying how long a file takes to read on this computer and how long until the folders are shown,
 * from the stored pace and the analysis's progress, kept counting down each second. `save()` stores this
 * run's pace once it has finished; `stop()` stops the countdown either way.
 */
function estimateLine() {
  const node = el("p", { className: "reading-estimate muted" });
  const started = performance.now();
  const elapsed = () => (performance.now() - started) / 1000;
  let average = null;
  // The files that need reading, and how many are read; the last pass is counted within the time per file.
  let reading = null;
  const draw = () => {
    node.textContent =
      reading?.total > 0
        ? describeEstimate(readingEstimate({ average, total: reading.total, finished: reading.finished, elapsed: elapsed() }), COMPUTER)
        : "";
  };
  readingPace()
    .then((pace) => {
      average = pace;
      draw();
    })
    // No history to go by: the estimate starts from the first guess.
    .catch(() => {});
  const ticking = setInterval(draw, 1000);
  return {
    node,
    update(progress) {
      if (progress.phase === "loading" || progress.phase === "reading") reading = progress;
      draw();
    },
    save() {
      const files = reading?.total ?? 0;
      // Not saved, the next estimate is only less close.
      if (files > 0) saveReadingPace(updatedAverage(average, { seconds: elapsed(), files })).catch(() => {});
    },
    stop: () => clearInterval(ticking),
  };
}

/**
 * The wait text, and the first time, a question whether to notify the user when the folders are ready. The
 * answer is kept in the config, so it's asked only once.
 */
function waitNote() {
  const text = el("p", { className: "note" }, WAIT_TEXT);
  const ask = el("p", { className: "notify-ask", hidden: true });
  const answer = (wanted) => {
    ask.hidden = true;
    text.textContent = wanted ? WAIT_TEXT : WAIT_TEXT_QUIET;
    // Not saved, the question comes again next time.
    answerNotifications(wanted).catch(() => {});
  };
  const yes = el("button", { type: "button", className: "small" }, "Notify me");
  const no = el("button", { type: "button", className: "small" }, "Don't notify");
  yes.addEventListener("click", () => answer(true));
  no.addEventListener("click", () => answer(false));
  ask.append(el("span", {}, "Notify you when the folders are ready?"), yes, no);
  notifications()
    .then((wanted) => {
      if (wanted === false) text.textContent = WAIT_TEXT_QUIET;
      ask.hidden = wanted !== null;
    })
    // The config can't be read: no question, and nothing is sent.
    .catch(() => {});
  return [text, ask];
}

export async function render(root, nav, result) {
  const screen = el("div");
  root.append(screen);
  const rootName = splitPath(result.root, pathSeparator).name;
  const sizes = new Map(result.entries.map((entry) => [entry.path, entry.size]));
  const modified = new Map(result.entries.map((entry) => [entry.path, entry.modified]));
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

  /**
   * Reads the files, with a counter, the time a file takes on this computer and a countdown to the folders.
   * A finished run updates that time; the user is notified when it ends while the app is in the background.
   */
  async function analyse() {
    const status = el("p", { className: "status" }, "Reading the files…");
    const bar = el("progress", { className: "analysis-progress" });
    const estimate = estimateLine();
    const cancel = el("button", { type: "button" }, "Cancel");
    let cancelled = false;
    // Goes back once the analysis has stopped, so it isn't still running when the user moves on.
    cancel.addEventListener("click", () => {
      cancelled = true;
      cancel.disabled = true;
      showStatus(status, "Stopping…");
      cancelAnalysis().catch((error) => showStatus(status, describeError(error), true));
    });
    screen.replaceChildren(...titled(status, bar, estimate.node, ...waitNote(), el("div", { className: "actions" }, cancel)));
    try {
      const { grouping, device } = await analyseLooseFiles((progress) => {
        if (cancelled) return;
        showStatus(status, describeAnalysisProgress(progress));
        bar.max = progress.total;
        bar.value = progress.finished;
        estimate.update(progress);
      });
      if (cancelled) return nav.home();
      estimate.save();
      notifyInBackground(`Folders are ready for ${rootName}`).catch(() => {});
      review(grouping, device);
    } catch (error) {
      if (cancelled) return nav.home();
      notifyInBackground(`Couldn't finish reading ${rootName}`).catch(() => {});
      failed(error);
    } finally {
      estimate.stop();
    }
  }

  function review(grouping, device) {
    const choices = initialChoices(grouping.folders);
    const summaryOf = (path) => grouping.summaries[path] ?? fileName(path);
    const kindOf = (path) => grouping.kinds[path];
    const modifiedOf = (path) => modified.get(path);
    const deviceNote = device ? el("p", { className: "note" }, `Files read on ${device}.`) : "";

    if (choices.length === 0) {
      screen.replaceChildren(
        ...titledAs(
          "No folders to suggest",
          el("p", { className: "lead" }, "No files here are alike enough to share a folder, so everything stays where it is."),
          el("div", { className: "actions" }, homeButton()),
          el("h2", {}, `Left where they are (${count(grouping.ungrouped.length)})`),
          ...leftLists(grouping.ungrouped, (path) => leftRow(path)),
          deviceNote,
        ),
      );
      return;
    }

    // The files being dragged, while a drag is on.
    let dragged = null;
    const left = new Set(grouping.ungrouped);
    // What the pane lists: a Suggested Folder's choice, or null for the files left where they are.
    let current = choices[0];
    // The files selected in the pane, and the one last clicked without Shift.
    let selection = new Set();
    let anchor = null;
    const sidebar = el("nav", { className: "folder-sidebar", ariaLabel: "Folders" });
    const pane = el("section", { className: "folder-pane" });
    const selectionBar = el("div", { className: "selection-bar", hidden: true });
    const menu = revealMenu();
    const displayName = (choice) => choice.name.trim() || "No name";
    // The Suggested Folder being renamed in the sidebar, if any: `{ choice, node }`, its name field kept
    // across redraws of the sidebar, so the name from before and the focus stay.
    let renaming = null;
    // Set while the sidebar is redrawn, when moving the name field can blur it.
    let redrawing = false;
    const move = (paths, to) => {
      for (const path of paths) moveFile(choices, left, path, to);
      selection = new Set();
      drawPane();
    };
    const show = (to) => {
      current = to;
      selection = new Set();
      anchor = null;
      drawPane();
    };
    /** Where the selected files can go from the pane: every folder but the one listed, or back where they are. */
    const destinations = () => [...(current ? [null] : []), ...choices.filter((choice) => choice !== current)];
    const destinationLabel = (to) => (to ? `Move to ${displayName(to)}` : "Leave where it is");

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

    const panels = new Map(choices.map((choice) => [choice, folderPanel(choice)]));
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
      el("div", { className: "folder-split" }, sidebar, pane),
      menu.node,
      deviceNote,
    );
    drawPane();

    /** The pane: the listed folder's settings and files, or the files left where they are. */
    function drawPane() {
      if (current) {
        const panel = panels.get(current);
        pane.replaceChildren(selectionBar, panel.node);
        panel.draw();
      } else {
        pane.replaceChildren(selectionBar, ...leftPanel());
        changed();
      }
    }

    /** Brings the sidebar, the selection and the header up to date after the pane changed. */
    function changed() {
      drawSidebar();
      drawSelection();
      drawHeader();
    }

    /**
     * The folders, files left where they are first; clicking one lists it, and a file dragged onto one goes
     * into it. The item that had focus keeps it.
     */
    function drawSidebar() {
      const focused = [...sidebar.children].indexOf(document.activeElement);
      const typing = renaming?.node.contains(document.activeElement);
      redrawing = true;
      sidebar.replaceChildren(
        sidebarItem(null, "Left where they are", left.size),
        el("hr", { className: "folder-sidebar-divider" }),
        ...choices.map(choiceItem),
      );
      redrawing = false;
      sidebar.children[focused]?.focus();
      if (typing) renaming.node.querySelector("input").focus();
    }

    /** The sidebar item for the Suggested Folder `choice`, or its name field while it's renamed. */
    function choiceItem(choice) {
      if (choice === renaming?.choice) return renaming.node;
      const className = nameMissing(choice) ? "unnamed" : choice.choice === "skip" ? "skipped" : "";
      return sidebarItem(choice, displayName(choice), choice.files.size, className);
    }

    /** A sidebar item listing `to` (null for the files left where they are); a Suggested Folder's is renamed by double-clicking it, or Return once it's listed, as in Finder. */
    function sidebarItem(to, label, n, className = "") {
      const node = el(
        "button",
        { type: "button", className: `folder-sidebar-item ${className}`, ariaCurrent: String(to === current), title: label },
        icon("folder"),
        el("span", { className: "folder-sidebar-name" }, label),
        el("span", { className: "muted folder-sidebar-count" }, count(n)),
      );
      // Clicking the listed one again leaves the sidebar as it is, so a double-click lands on the same item.
      node.addEventListener("click", () => to !== current && show(to));
      if (to) {
        node.addEventListener("dblclick", () => rename(to, node));
        node.addEventListener("keydown", (event) => {
          if (event.key !== "Enter" || to !== current) return;
          event.preventDefault();
          rename(to, node);
        });
      }
      dropTarget(node, to);
      return node;
    }

    /** Turns the sidebar item `node` of `choice` into a name field. */
    function rename(choice, node) {
      renaming = { choice, node: renameField(choice) };
      node.replaceWith(renaming.node);
      const input = renaming.node.querySelector("input");
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
      const node = el("div", { className: "folder-sidebar-renaming" }, el("div", { className: "folder-sidebar-item" }, icon("folder"), input), error);
      const named = () => {
        panels.get(choice).drawName();
        drawHeader();
      };
      const end = (save) => {
        // Already ended: replacing the field can blur it.
        if (renaming?.choice !== choice) return;
        if (!save) choice.name = before;
        renaming = null;
        const hadFocus = node.contains(document.activeElement);
        const item = choiceItem(choice);
        node.replaceWith(item);
        if (hadFocus) item.focus();
        named();
      };
      input.addEventListener("input", () => {
        choice.name = input.value;
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
      input.addEventListener("blur", () => redrawing || end(input.value.trim() !== ""));
      return node;
    }

    /** Marks the selected rows in the pane, and shows what can be done with them. */
    function drawSelection() {
      const rows = [...pane.querySelectorAll("[data-path]")];
      const shown = new Set(rows.map((row) => row.dataset.path));
      selection = new Set([...selection].filter((path) => shown.has(path)));
      for (const row of rows) row.classList.toggle("selected", selection.has(row.dataset.path));
      selectionBar.hidden = selection.size === 0;
      if (selection.size === 0) return;
      const targets = destinations();
      const to = el(
        "select",
        { ariaLabel: "Move the selected files" },
        el("option", { value: "", disabled: true, selected: true }, "Move to…"),
        ...targets.map((target, i) => el("option", { value: i }, target ? displayName(target) : "Leave where they are")),
      );
      to.value = "";
      to.addEventListener("change", () => move([...selection], targets[Number(to.value)]));
      const clear = el("button", { type: "button", className: "link" }, "Deselect");
      clear.addEventListener("click", () => {
        selection = new Set();
        drawSelection();
      });
      selectionBar.replaceChildren(el("span", {}, `${plural(selection.size, "file")} selected`), to, clear);
    }

    /**
     * Makes `row`, listing the file at `path`, selectable as in Finder, with a right-click menu to move the
     * selection, and draggable onto a folder in the sidebar, taking the whole selection when it is in it.
     */
    function selectable(row, path) {
      row.dataset.path = path;
      row.tabIndex = 0;
      row.draggable = true;
      const select = ({ toggle, range }) => {
        const order = [...pane.querySelectorAll("[data-path]")].map((node) => node.dataset.path);
        selection = clickSelection(selection, order, anchor, path, { toggle, range });
        if (!range) anchor = path;
        drawSelection();
      };
      row.addEventListener("click", (event) => {
        if (event.target.closest("button")) return;
        select({ toggle: event.metaKey || event.ctrlKey, range: event.shiftKey });
      });
      row.addEventListener("keydown", (event) => {
        if (event.key !== " " || event.target !== row) return;
        event.preventDefault();
        select({ toggle: true, range: false });
      });
      row.addEventListener("contextmenu", (event) => {
        event.preventDefault();
        if (!selection.has(path)) select({ toggle: false, range: false });
        const paths = [...selection];
        const items = destinations().map((to) => ({ label: destinationLabel(to), action: () => move(paths, to) }));
        menu.open(paths, row, event, items);
      });
      row.addEventListener("dragstart", (event) => {
        dragged = selection.has(path) ? [...selection] : [path];
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", dragged.join("\n"));
        for (const node of pane.querySelectorAll("[data-path]")) {
          if (dragged.includes(node.dataset.path)) node.classList.add("dragging");
        }
      });
      row.addEventListener("dragend", () => {
        dragged = null;
        for (const node of pane.querySelectorAll(".dragging")) node.classList.remove("dragging");
      });
      return row;
    }

    /** Makes `node` take the dragged files into the Suggested Folder `to`, or leave them where they are when `to` is null. */
    function dropTarget(node, to) {
      const holds = (path) => (to ? to.files.has(path) : left.has(path));
      const accepts = () => dragged !== null && dragged.some((path) => !holds(path));
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

    /** The files left where they are, by kind, for the pane. */
    function leftPanel() {
      return [
        el("h2", {}, `Left where they are (${count(left.size)})`),
        ...(left.size
          ? leftLists([...left], (path) => selectable(leftRow(path), path))
          : [el("p", { className: "note" }, "Drag a file onto “Left where they are” to leave it where it is.")]),
      ];
    }

    /** The pane for one Suggested Folder: its name, where it goes, what to do with it, and its files. */
    function folderPanel(choice) {
      const name = el("input", { type: "text", className: "folder-name", value: choice.name, ariaLabel: "Folder name", spellcheck: false });
      const nameError = el("p", { className: "status error folder-name-error" });
      const size = el("span", { className: "muted" });
      const where = el("p", { className: "folder-where" });
      const existingName = choice.folder.existing ? fileName(choice.folder.existing) : null;
      const options = [
        ...(existingName ? [["addTo", `Add to ${existingName}`]] : []),
        ["createHere", existingName ? "New folder here" : "Create here"],
        ["createIn", "Elsewhere…"],
        ["trashAll", "Trash all"],
        ["skip", "Skip"],
      ];
      const segments = options.map(([value, label]) => {
        const segment = el("button", { type: "button", role: "radio", className: "segment" }, label);
        segment.addEventListener("click", () => {
          // "Elsewhere…" always asks where, so it also changes the folder picked before.
          if (value === "createIn") return pickElsewhere();
          choice.choice = value;
          draw();
        });
        return [value, segment];
      });
      const pick = el("div", { className: "segmented", role: "radiogroup", ariaLabel: "What to do with this folder" }, ...segments.map(([, segment]) => segment));
      const drawPick = () => {
        for (const [value, segment] of segments) segment.ariaChecked = String(value === choice.choice);
      };
      const list = el("ul", { className: "folder-files", ariaLabel: "Files" });
      const empty = el("p", { className: "note" }, "No files. Drag files onto this folder in the list to add them.");
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
        draw();
      };
      name.addEventListener("input", () => {
        choice.name = name.value;
        drawName();
        changed();
      });

      /** The name and whether it's missing, also when it was renamed in the sidebar. */
      const drawName = () => {
        if (name.value !== choice.name) name.value = choice.name;
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
          const node = el("button", { type: "button", className: "small", disabled: choice.choice === "skip" }, label);
          node.addEventListener("click", set(next));
          return node;
        };
        const note = { trash: " · Trashed", takeOut: " · Stays where it is" }[state] ?? "";
        const actions = state === "inFolder" ? [button("Trash", "trash"), button("Take out", "takeOut")] : [button("Put back", "inFolder")];
        const row = el(
          "li",
          { className: "folder-file" },
          el("span", { className: "folder-file-name", title: summaryOf(path) }, fileName(path), el("span", { className: "muted" }, note)),
          el("span", { className: "folder-file-actions" }, ...actions),
        );
        row.classList.toggle("out", state !== "inFolder");
        return selectable(row, path);
      }

      function draw() {
        drawName();
        drawPick();
        drawWhere();
        const paths = [...choice.files.keys()];
        size.textContent = ` · ${plural(paths.length, "file")} · ${formatSize(paths.reduce((sum, path) => sum + (sizes.get(path) ?? 0), 0))}`;
        const shown = Math.min(paths.length, limit);
        list.classList.toggle("skipped", choice.choice === "skip");
        list.replaceChildren(...paths.slice(0, shown).map(fileRow));
        empty.hidden = paths.length > 0;
        drawMore(more, paths.length, shown, (next) => {
          limit = next;
          draw();
        });
        changed();
      }

      const node = el(
        "div",
        { className: "folder-panel" },
        el("div", { className: "folder-panel-head" }, icon("folder"), name, size),
        nameError,
        el("div", { className: "folder-panel-choice" }, pick, where),
        list,
        empty,
        more,
      );
      return { node, draw, drawName };
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

  try {
    const { state } = await groupingModelsStatus();
    if (state === "installed") analyse();
    else withoutModels();
  } catch (error) {
    failed(error);
  }
}
