// Plan: how much will change, where it goes, and why each file moves. One card per Reason
// picks which group's files are listed; the selected file's Reason is shown beside the list.
// The user ticks or unticks files, then moves the ticked ones, skips the step, or cancels. Nothing
// changes until then. This is the duplicate step: once it is run or skipped, the steps go on.
import { clearGroupingCards, describeError, pathSeparator, runPlan } from "../api.js";
import { el } from "../dom.js";
import { drawMore, revealMenu } from "../file-list.js";
import { formatSize, plural, splitPath } from "../format.js";
import { kindIcon } from "../kinds.js";
import { describeRunProgress, describeSummary } from "../outcomes.js";
import { initiallyTicked, tickState, tickableIds, toggleAll, totals } from "../plan-selection.js";
import {
  PAGE_SIZE,
  copyCounts,
  destinationFolder,
  fileName,
  folderLabel,
  groupByReason,
  newName,
  paging,
  selectedFile,
  visibleFiles,
} from "../plan-view.js";
import { busy, showStatus } from "../status.js";
import { describeScanProgress, fromDuplicateStep } from "../steps.js";

/**
 * What a Reason group's card and detail pane say. `why` and `tickLabel` take one of its Operations
 * and the folder it goes to.
 */
function reasonWords(group) {
  if (group.reason === "duplicates") {
    return {
      noun: "duplicate",
      explain: "Exact copies. The newest copy of each file stays where it is.",
      why: (op, folder) => `An exact copy of a newer file, so this one goes to ${folder}.`,
      tickLabel: () => "Move this copy",
    };
  }
  if (group.reason === "emptiedFolders") {
    return {
      noun: "empty folder",
      explain: "Folders left empty once their files move. Left out unless you tick them; only moved if still empty.",
      why: (op, folder) =>
        `Every file in it moves, so it would be left empty. If ticked, it goes to ${folder}, but only if nothing new is in it by then.`,
      tickLabel: () => "Move this empty folder",
    };
  }
  const rule = `Your Rule “${group.rule}”`;
  return {
    noun: "item",
    explain: `${rule} matched these files.`,
    why: (op, folder) =>
      ({
        trash: () => `${rule} sends this file to ${folder}.`,
        move: () => `${rule} moves this file to ${folder}${newName(op) ? `, renamed “${newName(op)}”` : ""}.`,
        createFolder: () => `${rule} moves files into ${folder}, which doesn't exist yet, so it is created.`,
      })[op.kind](),
    tickLabel: (op) => (op.kind === "createFolder" ? "Create this folder" : "Move this file"),
  };
}

/**
 * "Read every file again in the next step": forgets what the Grouping Models said about every file, so
 * Suggested Folders asks them afresh instead of reusing saved answers. Says how it went in `status`.
 */
function forgetReadingsNote(status) {
  const forget = el("button", { type: "button", className: "link" }, "Read every file again in the next step");
  forget.addEventListener("click", async () => {
    forget.disabled = true;
    try {
      await clearGroupingCards();
      showStatus(status, "Forgotten: the next step reads every file again.");
    } catch (error) {
      showStatus(status, describeError(error), true);
      forget.disabled = false;
    }
  });
  return el("p", { className: "note" }, "Suggested Folders reuses what was read last time. ", forget);
}

const SORT_NAMES = { size: "biggest first", name: "by name", folder: "by folder" };

export function render(root, nav, result, plan) {
  const rootName = splitPath(plan.root, pathSeparator).name;
  let ticked = initiallyTicked(plan.operations);

  const cancel = el("button", { type: "button" }, "Cancel");
  cancel.addEventListener("click", () => nav.scanResults(result));
  const status = el("p", { className: "status" });
  /** Goes on to the next step without running anything, with `buttons` disabled meanwhile. */
  const goOn = (buttons) =>
    busy(status, buttons, `Scanning ${result.root}…`, () =>
      nav.steps.continueAfterDuplicates(result.root, (progress) => showStatus(status, describeScanProgress(result.root, progress))),
    );

  if (plan.operations.length === 0) {
    const next = el("button", { type: "button", className: "primary" }, "Continue");
    next.addEventListener("click", () => goOn([next, cancel]));
    root.append(
      el("h1", {}, "No duplicates"),
      el("p", { className: "path" }, plan.root),
      el("p", { className: "lead" }, "No file here is an exact copy of another. Continue to scan again and suggest folders."),
      el("div", { className: "actions" }, cancel, next),
      status,
      forgetReadingsNote(status),
    );
    return;
  }

  const groups = groupByReason(plan.operations);
  const copies = copyCounts(plan.operations);
  const view = { group: groups[0], query: "", sort: "size", show: "all", limit: PAGE_SIZE, selectedId: null };

  // Header: how much is changing.
  const heading = el("h1", {});
  const run = el("button", { type: "button", className: "primary" });
  const skip = el("button", { type: "button", title: "Leave the duplicates as they are and go on" }, "Skip");
  // If the rescan fails, Run goes back to being disabled when nothing is ticked.
  skip.addEventListener("click", () => goOn([skip, run, cancel]).then(() => drawHeader()));
  const drawHeader = () => {
    const { count } = totals(plan.operations, ticked);
    heading.textContent = count ? `${plural(count, "file")} will move` : "Nothing will change";
    run.textContent = count ? `Move ${plural(count, "file")}` : "Nothing ticked";
    run.disabled = count === 0;
  };
  run.addEventListener("click", async () => {
    const files = plural(ticked.size, "file");
    const session = await busy(status, [run, skip, cancel], `Moving ${files}…`, () =>
      runPlan([...ticked], (progress) => showStatus(status, describeRunProgress(progress))),
    );
    if (session) nav.session(session.sessionId, fromDuplicateStep(result.root), describeSummary(session, "moved"));
    else drawHeader();
  });

  // Cards: where it is going, one per Reason; they also pick the listed group.
  const cards = groups.map((group) => card(group));
  function card(group) {
    const words = reasonWords(group);
    const ids = tickableIds(group.operations);
    // A Rule's folders to create come first; the card shows where its files go.
    const first = group.operations.find((op) => op.kind !== "createFolder") ?? group.operations[0];
    const { folder, insideRoot } = destinationFolder(first, plan.root, pathSeparator);
    const what = el("strong", {});
    const size = el("span", {});
    const pick = el(
      "button",
      { type: "button", className: "plan-card-pick" },
      el(
        "span",
        { className: "plan-card-line" },
        kindIcon(first.kind),
        what,
        el("span", { className: "muted" }, " · "),
        size,
        el("span", { className: "muted" }, " → "),
        // One item, so the note wraps with the folder's name rather than on its own.
        el("span", {}, el("strong", {}, folder), insideRoot ? el("span", { className: "muted" }, `, a folder inside ${rootName}`) : ""),
      ),
      el("span", { className: "plan-card-explain" }, words.explain),
    );
    const box = el("input", { type: "checkbox", ariaLabel: `Include every ${words.noun}`, disabled: ids.length === 0 });
    box.addEventListener("change", () => setTicked(toggleAll(ids, ticked)));
    const node = el("div", { className: "plan-card" }, pick, el("label", { className: "plan-include" }, box, "Include all"));
    pick.addEventListener("click", () => {
      if (view.group === group) return;
      view.group = group;
      view.limit = PAGE_SIZE;
      view.selectedId = null;
      drawCards();
      drawList();
    });
    const draw = () => {
      const current = view.group === group;
      node.classList.toggle("current", current);
      pick.ariaPressed = String(current);
      const tickedOps = group.operations.filter((op) => ticked.has(op.id));
      const all = plural(group.operations.length, words.noun);
      what.textContent = tickedOps.length === group.operations.length ? all : `${tickedOps.length} of ${all}`;
      size.textContent = formatSize(tickedOps.reduce((sum, op) => sum + op.size, 0));
      const state = tickState(ids, ticked);
      box.checked = state === "all";
      box.indeterminate = state === "some";
    };
    return { node, draw };
  }
  const drawCards = () => {
    for (const c of cards) c.draw();
  };

  // The chosen group's files: search, sort and Show, then one line per file.
  const search = el("input", { type: "search", className: "search", placeholder: "Search file names or folders", ariaLabel: "Search" });
  const sort = el(
    "select",
    { ariaLabel: "Sort" },
    el("option", { value: "size" }, "Biggest first"),
    el("option", { value: "name" }, "Name"),
    el("option", { value: "folder" }, "Folder"),
  );
  const show = el(
    "select",
    { ariaLabel: "Show" },
    el("option", { value: "all" }, "Show all"),
    el("option", { value: "ticked" }, "Show ticked"),
    el("option", { value: "unticked" }, "Show unticked"),
  );
  for (const [control, key] of [
    [search, "query"],
    [sort, "sort"],
    [show, "show"],
  ]) {
    control.addEventListener(control === search ? "input" : "change", () => {
      view[key] = control.value;
      view.limit = PAGE_SIZE;
      drawList();
    });
  }

  const listHeading = el("h2", {});
  const list = el("ul", { className: "plan-files", ariaLabel: "Files" });
  const more = el("div", { className: "plan-more" });
  const pane = el("aside", { className: "plan-pane", ariaLabel: "Selected file" });
  let rows = []; // [{ op, row, box }] for the files on screen

  const select = (op) => {
    view.selectedId = op.id;
    for (const r of rows) {
      const current = r.op === op;
      r.row.classList.toggle("selected", current);
      r.row.ariaCurrent = current ? "true" : null;
    }
    drawPane();
  };

  function fileRow(op, index) {
    // An Operation that can't happen can't be ticked; its row says why instead.
    const box = el("input", { type: "checkbox", ariaLabel: `Move ${fileName(op)}`, disabled: Boolean(op.error) });
    // Read after the name when the box gets focus, which is how a row is selected from the keyboard.
    box.setAttribute("aria-describedby", `plan-folder-${index} plan-size-${index}`);
    box.addEventListener("change", () => setTicked(withTick(op.id, box.checked)));
    box.addEventListener("focus", () => select(op));
    const n = copies(op);
    const folder = folderLabel(op, rootName);
    const row = el(
      "li",
      { className: "plan-file" },
      box,
      el(
        "span",
        { className: "plan-file-name", title: fileName(op) },
        fileName(op),
        n > 2 ? el("span", { className: "muted" }, ` · ${n} copies`) : "",
        el("span", { className: "plan-left-out" }, op.error ? " · Can't move" : " · Left out"),
      ),
      // Wrapped in left-to-right marks so the right-to-left box keeps the folder's end, not its slashes.
      el("span", { className: "plan-file-folder path", id: `plan-folder-${index}`, title: fullFolder(op) }, `‎${folder}‎`),
      el("span", { className: "plan-file-size", id: `plan-size-${index}` }, formatSize(op.size)),
    );
    // WebKit doesn't focus a tick box when it is clicked, so a click selects the row itself.
    row.addEventListener("click", (event) => {
      select(op);
      if (event.target !== box) box.focus();
    });
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      select(op);
      const keeper = op.reason.type === "duplicate" ? [[plan.root, ...op.reason.keeper].join(pathSeparator)] : [];
      menu.open([op.source, ...keeper], row, event);
    });
    if (op.error) row.classList.add("blocked");
    return { op, row, box };
  }
  const fullFolder = (op) => [plan.root, ...op.relativePath.slice(0, -1)].join(pathSeparator);

  // Right-clicking a file opens a menu with "Show in folder", which selects the file and, for a
  // duplicate, its Keeper too.
  const menu = revealMenu();

  // ↑ and ↓ move between files; Space ticks the focused one (it is a tick box).
  list.addEventListener("keydown", (event) => {
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
    const at = rows.findIndex((r) => r.box === event.target);
    if (!step || at === -1) return;
    event.preventDefault();
    rows[Math.min(rows.length - 1, Math.max(0, at + step))]?.box.focus();
  });

  function drawList() {
    const words = reasonWords(view.group);
    listHeading.textContent = `${plural(view.group.operations.length, words.noun)}, ${SORT_NAMES[view.sort]}`;
    const files = visibleFiles(view.group.operations, view, ticked);
    const { shown } = paging(files.length, view.limit);
    const focusedAt = rows.findIndex((r) => r.box === document.activeElement);
    const focused = rows[focusedAt]?.op.id;
    const paneFocused = pane.contains(document.activeElement);
    rows = files.slice(0, shown).map(fileRow);
    list.replaceChildren(...(rows.length ? rows.map((r) => r.row) : [el("li", { className: "plan-none" }, "No files match.")]));
    drawTicks();

    drawMore(more, files.length, shown, (limit) => {
      view.limit = limit;
      drawList();
    });

    // A file ticked or unticked under Show Ticked or Unticked leaves the list: the file that
    // took its place is selected and focus stays where it was, or goes to Show if none is left.
    const stayed = focused && (rows.find((r) => r.op.id === focused) ?? rows[Math.min(focusedAt, rows.length - 1)]);
    const current = stayed?.op ?? selectedFile(rows.map((r) => r.op), view.selectedId);
    pane.hidden = !current;
    if (current) select(current);
    if (stayed) stayed.box.focus();
    else if (paneFocused && current) paneBox.focus();
    else if (focused || paneFocused) show.focus();
  }

  /** Brings every file's tick box, and its left-out mark, in step with `ticked`. */
  function drawTicks() {
    for (const { op, row, box } of rows) {
      box.checked = ticked.has(op.id);
      row.classList.toggle("unticked", !box.checked);
    }
  }

  let paneFile = null;
  let paneBox = null;
  function drawPane() {
    const op = rows.find((r) => r.op.id === view.selectedId)?.op;
    if (!op) return;
    if (op === paneFile) {
      paneBox.checked = ticked.has(op.id);
      return;
    }
    paneFile = op;
    const words = reasonWords(view.group);
    const box = el("input", { type: "checkbox", checked: ticked.has(op.id), disabled: Boolean(op.error) });
    box.addEventListener("change", () => setTicked(withTick(op.id, box.checked)));
    paneBox = box;
    pane.replaceChildren(
      el("h3", { className: "plan-pane-name" }, fileName(op)),
      el("p", { className: "muted" }, op.isFolder ? "Folder" : formatSize(op.size)),
      el("p", {}, words.why(op, destinationFolder(op, plan.root, pathSeparator).folder)),
      op.error ? el("p", { className: "status error" }, `Can't be done: ${op.error}.`) : "",
      op.reason.type === "duplicate"
        ? el("p", { className: "plan-original" }, "Original: ", el("span", { className: "path" }, [rootName, ...op.reason.keeper].join("/")))
        : "",
      el("label", { className: "plan-include" }, box, words.tickLabel(op)),
    );
  }

  const withTick = (id, on) => {
    const next = new Set(ticked);
    if (on) next.add(id);
    else next.delete(id);
    return next;
  };
  const setTicked = (next) => {
    ticked = next;
    drawHeader();
    drawCards();
    // Under Show all no file comes or goes, so the list keeps its place and focus.
    if (view.show === "all") {
      drawTicks();
      drawPane();
    } else drawList();
  };

  root.append(
    el(
      "header",
      { className: "plan-head" },
      el(
        "div",
        {},
        heading,
        el("p", { className: "lead" }, `Nothing is deleted. Everything stays inside ${rootName}, and you can undo it from History.`),
      ),
      el("div", { className: "actions" }, cancel, skip, run),
    ),
    status,
    forgetReadingsNote(status),
    el("div", { className: "plan-cards" }, ...cards.map((c) => c.node)),
    el(
      "div",
      { className: "plan-split" },
      el("section", { className: "plan-list" }, listHeading, el("div", { className: "filters" }, search, sort, show), list, more, menu.node),
      pane,
    ),
  );
  drawHeader();
  drawCards();
  drawList();
}
