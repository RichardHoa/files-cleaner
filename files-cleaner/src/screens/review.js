// Review: the one screen after the scan, holding the whole Plan (ADR 0005), laid out like Mail. A sidebar
// lists the Duplicates, then the files left where they are and each Suggested Folder, then the Emptied
// Folders, and the pane beside it lists the one picked. The Duplicate Groups show at once. The loose
// files are read in the background meanwhile, which needs the Grouping Models (without them, they're
// offered in the Suggested Folders' place), and the Suggested Folders are filled in once that's done.
// Files are selected as in Finder, then moved by dragging them onto a folder in the sidebar, from their
// right-click menu, or with "Move to…". Approve waits for the reading, which can be stopped early to group
// only what was read, then runs the whole Plan as one Session; Cancel stops the reading and goes Home with
// nothing changed.
import {
  analyseLooseFiles,
  answerNotifications,
  buildReviewPlan,
  cancelAnalysis,
  describeError,
  groupingModelsStatus,
  notifications,
  notifyInBackground,
  pathSeparator,
  pickFolder,
  previewReviewPlan,
  readingPace,
  runPlan,
  saveReadingPace,
  stopReading,
} from "../api.js";
import { el } from "../dom.js";
import { drawMore, revealMenu } from "../file-list.js";
import { count, formatDate, formatSize, plural, splitPath } from "../format.js";
import { icon } from "../icons.js";
import { groupingModelsPanel } from "../models-panel.js";
import { describeRunProgress, describeSummary } from "../outcomes.js";
import { tickState, toggleAll } from "../plan-selection.js";
import { progressBar } from "../progress-bar.js";
import { describeEstimate, readingEstimate, updatedAverage } from "../reading-estimate.js";
import {
  byKindNewestFirst,
  initialChoices,
  initialDuplicates,
  leftCopies,
  moveFile,
  nameMissing,
  placeKeepers,
  planChoices,
  planDuplicates,
  setCopy,
  totals,
  trashedCopies,
} from "../review-choices.js";
import { clickSelection } from "../row-selection.js";
import { busy, showStatus } from "../status.js";

/** How many of a card's files, or Duplicate Groups, are listed before "Show more". */
const FILES_SHOWN = 50;
/** How long the Emptied Folders wait after a change before they're worked out again, in ms. */
const EMPTIED_DELAY = 300;
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

export async function render(root, nav, result, duplicateGroups) {
  const rootName = splitPath(result.root, pathSeparator).name;
  const sizes = new Map(result.entries.map((entry) => [entry.path, entry.size]));
  const modified = new Map(result.entries.map((entry) => [entry.path, entry.modified]));
  const totalSize = result.entries.reduce((sum, entry) => sum + entry.size, 0);
  const duplicates = initialDuplicates(duplicateGroups);
  // The copy read for each copy in a Duplicate Group, its first, so a Keeper the user picks shows what was read.
  const readAs = new Map(duplicateGroups.flatMap((group) => group.copies.map((copy) => [copy.path, group.copies[0].path])));

  // The Suggested Folders: "reading", "offer" (without the Grouping Models), "failed" or "ready". Until
  // they're ready, the pane for them shows `waiting`: the reading, the offer or the error.
  let folders = "reading";
  let waiting = [el("h2", {}, "Suggested Folders"), el("p", { className: "status" }, "Reading the files…")];
  let grouping = null;
  let choices = [];
  const left = new Set();
  let panels = new Map();
  const summaryOf = (path) => grouping?.summaries[readAs.get(path) ?? path] ?? fileName(path);
  const kindOf = (path) => grouping?.kinds[readAs.get(path) ?? path];
  const modifiedOf = (path) => modified.get(path);
  // The Plan's Emptied Folder Operations as last worked out, and the ones ticked, by ID.
  let emptied = [];
  let emptiedTicked = new Set();

  // What the pane lists: "duplicates", "folders" (until they're ready), "left" (the files left where they
  // are), "emptied", or a Suggested Folder's choice.
  let current = duplicates.length > 0 ? "duplicates" : "folders";
  // The files being dragged, while a drag is on.
  let dragged = null;
  // The files selected in the pane, and the one last clicked without Shift.
  let selection = new Set();
  let anchor = null;
  let duplicatesShown = FILES_SHOWN;
  const sidebar = el("nav", { className: "folder-sidebar", ariaLabel: "Plan" });
  const pane = el("section", { className: "folder-pane" });
  const selectionBar = el("div", { className: "selection-bar", hidden: true });
  const footer = el("div");
  const menu = revealMenu();
  const displayName = (choice) => choice.name.trim() || "No name";
  // The Suggested Folder being renamed in the sidebar, if any: `{ choice, node }`, its name field kept
  // across redraws of the sidebar, so the name from before and the focus stay.
  let renaming = null;
  // Set while the sidebar is redrawn, when moving the name field can blur it.
  let redrawing = false;
  // Set while the files are being read, and once Cancel was clicked then: the screen goes Home once
  // the reading has stopped. Once Stop reading was clicked, the files read so far are grouped instead.
  let analysing = false;
  let cancelled = false;
  let stopped = false;
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
  const destinations = () => [...(current === "left" ? [] : [null]), ...choices.filter((choice) => choice !== current)];
  const destinationLabel = (to) => (to ? `Move to ${displayName(to)}` : "Leave where it is");

  const heading = el("h1", {});
  const run = el("button", { type: "button", className: "primary" });
  const cancel = el("button", { type: "button" }, "Cancel");
  cancel.addEventListener("click", () => {
    if (!analysing) return nav.home();
    // Goes Home once the reading has stopped, so it isn't still running when the user moves on.
    cancelled = true;
    cancel.disabled = true;
    showStatus(status, "Stopping…");
    cancelAnalysis().catch((error) => showStatus(status, describeError(error), true));
  });
  const status = el("p", { className: "status" });
  const drawHeader = () => {
    const { moved, trashed } = totals(choices, duplicates);
    const n = moved + trashed;
    heading.textContent = n ? `${plural(n, "file")} will move` : "Nothing will change";
    if (folders === "reading") {
      run.textContent = "Waiting for folders…";
      run.disabled = true;
      return;
    }
    run.textContent = n ? `Move ${plural(n, "file")}` : "Nothing to move";
    run.disabled = n === 0 || choices.some(nameMissing);
  };

  run.addEventListener("click", async () => {
    clearTimeout(emptiedTimer);
    const session = await busy(status, [run, cancel], "Planning…", async () => {
      const plan = await buildReviewPlan(planDuplicates(duplicates), planChoices(choices));
      const blocked = plan.operations.filter((op) => op.error && op.reason.type === "suggestedFolder");
      if (blocked.length > 0) {
        const { reason, error } = blocked[0];
        throw new Error(
          `${plural(blocked.length, "file")} can't be moved into “${reason.folder}”: ${error}. Choose another place for that folder, or skip it.`,
        );
      }
      const approved = plan.operations
        .filter((op) => (op.reason.type === "emptiedFolder" ? emptiedTicked.has(op.id) && !op.error : op.ticked))
        .map((op) => op.id);
      return runPlan(approved, (progress) => showStatus(status, describeRunProgress(progress)));
    });
    if (session) nav.session(session.sessionId, null, describeSummary(session, "moved"));
    else drawHeader();
  });

  // The Emptied Folders depend on everything the Plan moves, so they are worked out by the backend again
  // a moment after each change. Only the latest answer counts.
  let emptiedTimer = 0;
  let emptiedAsked = 0;
  const refreshEmptied = () => {
    clearTimeout(emptiedTimer);
    emptiedTimer = setTimeout(async () => {
      if (!pane.isConnected) return;
      const asked = ++emptiedAsked;
      try {
        const plan = await previewReviewPlan(planDuplicates(duplicates), planChoices(choices));
        if (asked !== emptiedAsked || !pane.isConnected) return;
        emptied = plan.operations.filter((op) => op.reason.type === "emptiedFolder");
        const ids = new Set(emptied.map((op) => op.id));
        emptiedTicked = new Set([...emptiedTicked].filter((id) => ids.has(id)));
        if (current === "emptied") drawPane();
        else drawSidebar();
      } catch {
        // A name that can't be a folder name: the Emptied Folders stay as they were until it's fixed.
      }
    }, EMPTIED_DELAY);
  };

  root.append(
    el(
      "header",
      { className: "plan-head" },
      el(
        "div",
        {},
        heading,
        el("p", { className: "path root-path" }, icon("folder"), result.root),
        el(
          "p",
          { className: "lead" },
          `${plural(result.entries.length, "item")} · ${formatSize(totalSize)} scanned. Nothing is deleted, and you can undo it all from History.`,
        ),
      ),
      el("div", { className: "actions" }, cancel, run),
    ),
    status,
    el("div", { className: "folder-split" }, sidebar, pane),
    menu.node,
    footer,
  );
  drawPane();

  /** The pane: the listed section of the Plan. */
  function drawPane() {
    if (choices.includes(current)) {
      const panel = panels.get(current);
      pane.replaceChildren(selectionBar, panel.node);
      panel.draw();
      return;
    }
    const sections = {
      duplicates: duplicatesPanel,
      folders: () => waiting,
      left: () => [selectionBar, ...leftPanel()],
      emptied: emptiedPanel,
    };
    pane.replaceChildren(...sections[current]());
    changed();
  }

  /** Brings the sidebar, the selection, the header and the Emptied Folders up to date after the pane changed. */
  function changed() {
    drawSidebar();
    drawSelection();
    drawHeader();
    refreshEmptied();
  }

  /**
   * The Plan in order: Duplicates, then the files left where they are and each Suggested Folder (or one
   * entry for them until they're ready), then the Emptied Folders. Clicking one lists it, and a file
   * dragged onto a folder goes into it. The item that had focus keeps it.
   */
  function drawSidebar() {
    const focused = [...sidebar.children].indexOf(document.activeElement);
    const typing = renaming?.node.contains(document.activeElement);
    redrawing = true;
    sidebar.replaceChildren(
      sidebarItem("duplicates", "Duplicates", duplicates.reduce((n, duplicate) => n + trashedCopies(duplicate), 0), "", "copies"),
      el("hr", { className: "folder-sidebar-divider" }),
      ...(folders === "ready"
        ? [sidebarItem("left", "Left where they are", left.size), ...choices.map(choiceItem)]
        : [sidebarItem("folders", "Suggested Folders", null)]),
      ...(emptied.length > 0
        ? [el("hr", { className: "folder-sidebar-divider" }), sidebarItem("emptied", "Emptied Folders", emptied.length)]
        : []),
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

  /**
   * A sidebar item listing `section` with `n` items, or no count when `n` is null. A Suggested Folder's
   * is renamed by double-clicking it, or Return once it's listed, as in Finder; files can be dragged onto
   * it, and onto the files left where they are.
   */
  function sidebarItem(section, label, n, className = "", iconName = "folder") {
    const node = el(
      "button",
      { type: "button", className: `folder-sidebar-item ${className}`, ariaCurrent: String(section === current), title: label },
      icon(iconName),
      el("span", { className: "folder-sidebar-name" }, label),
      el("span", { className: "muted folder-sidebar-count" }, n == null ? "" : count(n)),
    );
    // Clicking the listed one again leaves the sidebar as it is, so a double-click lands on the same item.
    node.addEventListener("click", () => section !== current && show(section));
    if (choices.includes(section)) {
      node.addEventListener("dblclick", () => rename(section, node));
      node.addEventListener("keydown", (event) => {
        if (event.key !== "Enter" || section !== current) return;
        event.preventDefault();
        rename(section, node);
      });
      dropTarget(node, section);
    } else if (section === "left") dropTarget(node, null);
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

  /**
   * The Duplicate Groups, each with its name, how many copies and the space freed, then every copy in its
   * folder, the Keeper first. A copy's right-click menu shows it in the file manager.
   */
  function duplicatesPanel() {
    if (duplicates.length === 0) return [el("h2", {}, "Duplicates"), el("p", { className: "note" }, "No file here is an exact copy of another.")];
    const trashed = duplicates.reduce((n, duplicate) => n + trashedCopies(duplicate), 0);
    const leaving = duplicates.reduce((n, duplicate) => n + leftCopies(duplicate), 0);
    const freed = duplicates.reduce((sum, duplicate) => sum + duplicate.group.size * trashedCopies(duplicate), 0);
    const shown = Math.min(duplicates.length, duplicatesShown);
    const more = el("div", { className: "plan-more" });
    drawMore(more, duplicates.length, shown, (limit) => {
      duplicatesShown = limit;
      drawPane();
    });
    return [
      el("h2", {}, "Duplicates"),
      el(
        "p",
        { className: "muted" },
        `${count(trashed)} ${trashed === 1 ? "copy" : "copies"} to trash`,
        leaving ? `, ${count(leaving)} left where ${leaving === 1 ? "it is" : "they are"}` : "",
        `, of ${plural(duplicates.length, "file")} · ${formatSize(freed)} freed`,
      ),
      el("ul", { className: "duplicate-groups", ariaLabel: "Duplicate Groups" }, ...duplicates.slice(0, shown).map(duplicateGroup)),
      more,
    ];
  }

  /** A Duplicate Group: its Keeper's name, how many copies, the space freed, then each copy, the Keeper first. */
  function duplicateGroup(duplicate) {
    const { group, copies } = duplicate;
    const keeperFirst = [...group.copies].sort((a, b) => (copies.get(b.path) === "keeper") - (copies.get(a.path) === "keeper"));
    const name = fileName(keeperFirst[0].path);
    return el(
      "li",
      { className: "duplicate-group" },
      el(
        "div",
        { className: "duplicate-group-head" },
        el("span", { className: "duplicate-group-name", title: name }, name),
        el("span", { className: "muted" }, ` · ${count(group.copies.length)} copies · ${formatSize(group.size * trashedCopies(duplicate))} freed`),
      ),
      el("ul", { className: "duplicate-copies" }, ...keeperFirst.map((copy) => duplicateCopy(duplicate, copy, name))),
    );
  }

  /**
   * One copy of the Duplicate Group `duplicate`, whose Keeper is named `name`: Keep, Trash or Leave, its
   * folder, and its own name if it differs. Keep makes it the Keeper, trashing the one before; the Keeper
   * can't be trashed or left itself.
   */
  function duplicateCopy(duplicate, copy, name) {
    const fate = duplicate.copies.get(copy.path);
    const { name: own, parent } = splitPath(copy.path, pathSeparator);
    const segments = [
      ["keeper", "Keep"],
      ["trash", "Trash"],
      ["leave", "Leave"],
    ].map(([value, label]) => {
      const segment = el(
        "button",
        { type: "button", role: "radio", className: "segment", ariaChecked: String(value === fate), disabled: fate === "keeper" && value !== "keeper" },
        label,
      );
      segment.addEventListener("click", () => {
        if (value === fate) return;
        setCopy(duplicate, copy.path, value, choices, left);
        drawPane();
        // The copy's row is drawn again, maybe elsewhere in its group: the focus goes with it.
        pane.querySelector(`[data-copy="${CSS.escape(copy.path)}"] .segment[aria-checked="true"]`)?.focus();
      });
      return segment;
    });
    const row = el(
      "li",
      { className: `duplicate-copy ${fate}`, tabIndex: 0 },
      el("div", { className: "segmented", role: "radiogroup", ariaLabel: `What to do with ${own}` }, ...segments),
      el("span", { className: "duplicate-copy-folder path", title: copy.path }, parent, own !== name ? el("span", { className: "muted" }, ` · ${own}`) : ""),
    );
    row.dataset.copy = copy.path;
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      menu.open([copy.path], row, event);
    });
    return row;
  }

  /** The files left where they are, by kind, for the pane. */
  function leftPanel() {
    if (choices.length === 0) {
      return [
        el("h2", {}, `Left where they are (${count(left.size)})`),
        el("p", { className: "note" }, "No files here are alike enough to share a folder, so everything stays where it is."),
        ...leftLists([...left], (path) => leftRow(path)),
      ];
    }
    return [
      el("h2", {}, `Left where they are (${count(left.size)})`),
      ...(left.size
        ? leftLists([...left], (path) => selectable(leftRow(path), path))
        : [el("p", { className: "note" }, "Drag a file onto “Left where they are” to leave it where it is.")]),
    ];
  }

  /** The folders the Plan leaves empty, each unticked until the user ticks it. */
  function emptiedPanel() {
    const ids = emptied.filter((op) => !op.error).map((op) => op.id);
    const all = el("input", { type: "checkbox", disabled: ids.length === 0 });
    const boxes = [];
    const drawTicks = () => {
      const state = tickState(ids, emptiedTicked);
      all.checked = state === "all";
      all.indeterminate = state === "some";
      for (const [op, box] of boxes) box.checked = emptiedTicked.has(op.id);
    };
    all.addEventListener("change", () => {
      emptiedTicked = toggleAll(ids, emptiedTicked);
      drawTicks();
    });
    const rows = emptied.map((op) => {
      const folder = op.relativePath.join("/");
      const box = el("input", { type: "checkbox", disabled: Boolean(op.error), ariaLabel: `Move the empty folder ${folder}` });
      box.addEventListener("change", () => {
        emptiedTicked = new Set(emptiedTicked);
        if (box.checked) emptiedTicked.add(op.id);
        else emptiedTicked.delete(op.id);
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
      el("h2", {}, "Emptied Folders"),
      el("p", { className: "muted" }, "Folders left empty once their files move. Left out unless you tick them; only moved if still empty."),
      el("label", { className: "plan-include" }, all, "Include all"),
      emptied.length ? el("ul", { className: "folder-files" }, ...rows) : el("p", { className: "note" }, "No folder is left empty."),
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

  /** Shows `nodes` in the Suggested Folders' pane until they're ready, with `state` as `folders`. */
  function waitFor(state, nodes) {
    folders = state;
    waiting = [el("h2", {}, "Suggested Folders"), ...nodes];
    if (current === "folders") drawPane();
    else changed();
  }

  /**
   * Reads the files, with a counter, the time a file takes on this computer and a countdown to the folders,
   * while the rest of the Plan can be reviewed. A finished run updates that time; the user is notified when
   * it ends while the app is in the background.
   */
  async function analyse() {
    const progress = el("p", { className: "status" }, "Reading the files…");
    const bar = progressBar();
    const estimate = estimateLine();
    const stop = el("button", { type: "button", className: "small" }, "Stop reading");
    stop.addEventListener("click", () => {
      stopped = true;
      stop.disabled = true;
      showStatus(progress, "Stopping after this file…");
      stopReading().catch((error) => showStatus(progress, describeError(error), true));
    });
    waitFor("reading", [progress, bar.node, estimate.node, el("p", {}, stop), ...waitNote()]);
    analysing = true;
    try {
      const analysis = await analyseLooseFiles((update) => {
        if (cancelled || stopped) return;
        // Every file is read by the last pass: stopping then would only skip it.
        if (update.phase === "placing") stop.hidden = true;
        showStatus(progress, describeAnalysisProgress(update));
        // Moves back and forth while the models load, as nothing is read meanwhile.
        bar.set(update.finished, update.phase === "loading" ? 0 : update.total);
        estimate.update(update);
      });
      if (cancelled) return nav.home();
      // A stopped run says nothing of this computer's pace, and the user is there to see it end.
      if (!stopped) {
        estimate.save();
        notifyInBackground(`Folders are ready for ${rootName}`).catch(() => {});
      }
      ready(analysis);
    } catch (error) {
      if (cancelled) return nav.home();
      if (!stopped) notifyInBackground(`Couldn't finish reading ${rootName}`).catch(() => {});
      waitFor("failed", [el("p", { className: "status error" }, describeError(error))]);
    } finally {
      analysing = false;
      estimate.stop();
    }
  }

  /** Fills in the Suggested Folders and the files left where they are, from the analysis, each Keeper picked meanwhile in the place of the copy read. */
  function ready(analysis) {
    grouping = analysis.grouping;
    choices = initialChoices(grouping.folders);
    for (const path of grouping.ungrouped) left.add(path);
    placeKeepers(choices, left, duplicates);
    panels = new Map(choices.map((choice) => [choice, folderPanel(choice)]));
    folders = "ready";
    if (analysis.device) footer.replaceChildren(el("p", { className: "note" }, `Files read on ${analysis.device}.`));
    if (current === "folders") show(choices[0] ?? "left");
    else changed();
  }

  try {
    const { state } = await groupingModelsStatus();
    if (state === "installed") analyse();
    else {
      waitFor("offer", [
        groupingModelsPanel(analyse),
        el("p", { className: "note" }, "Sorting by kind, name and date, without the grouping models, is coming."),
      ]);
    }
  } catch (error) {
    waitFor("failed", [el("p", { className: "status error" }, describeError(error))]);
  }
}
