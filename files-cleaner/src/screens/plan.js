// Plan: how much will change, where it goes, and why each file moves. One card per Reason
// picks which group's files are listed; the selected file's Reason is shown beside the list.
// The user ticks or unticks files, then moves the ticked ones or cancels. Nothing changes until then.
import { pathSeparator, runPlan } from "../api.js";
import { el } from "../dom.js";
import { drawMore, revealMenu } from "../file-list.js";
import { formatSize, plural, splitPath } from "../format.js";
import { kindIcon } from "../kinds.js";
import { describeSummary } from "../outcomes.js";
import { tickState, toggleAll, totals } from "../plan-selection.js";
import { PAGE_SIZE, copyCounts, fileName, folderLabel, groupByReason, paging, selectedFile, visibleFiles } from "../plan-view.js";
import { busy } from "../status.js";

/** What each Reason's card and detail pane say. `destination` is the folder its files go to. */
const REASON_WORDS = {
  duplicates: {
    noun: "duplicate",
    explain: "Exact copies. The newest copy of each file stays where it is.",
    why: (destination) => `An exact copy of a newer file, so this one goes to ${destination}.`,
    tickLabel: "Move this copy",
  },
};

const SORT_NAMES = { size: "biggest first", name: "by name", folder: "by folder" };

export function render(root, nav, result, plan) {
  const rootName = splitPath(plan.root, pathSeparator).name;
  let ticked = new Set(plan.operations.map((op) => op.id));

  const cancel = el("button", { type: "button" }, "Cancel");
  cancel.addEventListener("click", () => nav.scanResults(result));

  if (plan.operations.length === 0) {
    root.append(
      el("h1", {}, "Plan"),
      el("p", { className: "path" }, plan.root),
      el("p", { className: "lead" }, "Nothing to do: no duplicates were found."),
      el("div", { className: "actions" }, cancel),
    );
    return;
  }

  const groups = groupByReason(plan.operations);
  const copies = copyCounts(plan.operations);
  const view = { group: groups[0], query: "", sort: "size", show: "all", limit: PAGE_SIZE, selectedId: null };

  // Header: how much is changing.
  const heading = el("h1", {});
  const run = el("button", { type: "button", className: "primary" });
  const status = el("p", { className: "status" });
  const drawHeader = () => {
    const { count } = totals(plan.operations, ticked);
    heading.textContent = count ? `${plural(count, "file")} will move` : "Nothing will change";
    run.textContent = count ? `Move ${plural(count, "file")}` : "Nothing ticked";
    run.disabled = count === 0;
  };
  run.addEventListener("click", async () => {
    const files = plural(ticked.size, "file");
    const session = await busy(status, [run, cancel], `Moving ${files}…`, () => runPlan([...ticked]));
    if (session) nav.session(session.sessionId, null, describeSummary(session, "moved"));
    else drawHeader();
  });

  // Cards: where it is going, one per Reason; they also pick the listed group.
  const cards = groups.map((group) => card(group));
  function card(group) {
    const words = REASON_WORDS[group.reason];
    const ids = group.operations.map((op) => op.id);
    const destination = group.operations[0].destination[0];
    const what = el("strong", {});
    const size = el("span", {});
    const pick = el(
      "button",
      { type: "button", className: "plan-card-pick" },
      el(
        "span",
        { className: "plan-card-line" },
        kindIcon(group.operations[0].kind),
        what,
        el("span", { className: "muted" }, " · "),
        size,
        el("span", { className: "muted" }, " → "),
        // One item, so the note wraps with the folder's name rather than on its own.
        el("span", {}, el("strong", {}, destination), el("span", { className: "muted" }, `, a folder inside ${rootName}`)),
      ),
      el("span", { className: "plan-card-explain" }, words.explain),
    );
    const box = el("input", { type: "checkbox", ariaLabel: `Include every ${words.noun}` });
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
      what.textContent = tickedOps.length === ids.length ? all : `${tickedOps.length} of ${all}`;
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
    const box = el("input", { type: "checkbox", ariaLabel: `Move ${fileName(op)}` });
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
        el("span", { className: "plan-left-out" }, " · Left out"),
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
      menu.open(op.keeper ? [op.source, [plan.root, ...op.keeper].join(pathSeparator)] : [op.source], row, event);
    });
    return { op, row, box };
  }
  const fullFolder = (op) => [plan.root, ...op.relativePath.slice(0, -1)].join(pathSeparator);

  // Right-clicking a file opens a menu with "Show in Finder", which selects the file and, for a
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
    const words = REASON_WORDS[view.group.reason];
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
    const words = REASON_WORDS[view.group.reason];
    const box = el("input", { type: "checkbox", checked: ticked.has(op.id) });
    box.addEventListener("change", () => setTicked(withTick(op.id, box.checked)));
    paneBox = box;
    pane.replaceChildren(
      el("h3", { className: "plan-pane-name" }, fileName(op)),
      el("p", { className: "muted" }, formatSize(op.size)),
      el("p", {}, words.why(op.destination[0])),
      op.keeper ? el("p", { className: "plan-original" }, "Original: ", el("span", { className: "path" }, [rootName, ...op.keeper].join("/"))) : "",
      el("label", { className: "plan-include" }, box, words.tickLabel),
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
      el("div", { className: "actions" }, cancel, run),
    ),
    status,
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
