// Session: what one past Session did and where its files are now. One card per group (Moved,
// Put back, Needs attention) picks which files are listed; the selected file is shown beside the
// list. Files can be put back, or moved again, all at once, ticked, or one at a time. Right after the
// duplicate Session it also offers Continue, to the next step.
import { describeError, pathSeparator, redoOperations, revealInFileManager, sessionDetails, undo } from "../api.js";
import { el } from "../dom.js";
import { drawMore, revealMenu } from "../file-list.js";
import { count, formatDate, formatSize, plural, splitPath } from "../format.js";
import { describeSummary } from "../outcomes.js";
import { tickState, toggleAll } from "../plan-selection.js";
import { PAGE_SIZE, fileName, folderLabel, paging, selectedFile, visibleFiles } from "../plan-view.js";
import { groupByState, holdingName, paneSentence, sessionHeading } from "../session-view.js";
import { busy, showStatus } from "../status.js";
import { describeScanProgress } from "../steps.js";

/** "3 moved": how many files a group has, as its card and list heading say it. */
const GROUP_COUNTS = {
  moved: (n) => `${count(n)} moved`,
  putBack: (n) => `${count(n)} put back`,
  attention: (n) => `${count(n)} ${n === 1 ? "needs" : "need"} attention`,
};

/** What can be done to a group's files, and how it is worded. Needs attention has no action. */
const ACTIONS = {
  moved: {
    bulk: (n) => `Put back ${plural(n, "file")}`,
    single: "Put back",
    tickLabel: (name) => `Put back ${name}`,
    busy: "Putting back…",
    done: "put back",
    call: (sessionId, ids) => undo(sessionId, ids),
  },
  putBack: {
    bulk: (n) => `Move ${plural(n, "file")} again`,
    single: "Move again",
    tickLabel: (name) => `Move ${name} again`,
    busy: "Moving…",
    done: "moved again",
    call: (sessionId, ids) => redoOperations(sessionId, ids),
  },
};

const SORT_NAMES = { size: "biggest first", name: "by name", folder: "by folder" };

/** Where the file is now, for Show in folder: null once it is gone for good. */
function whereNow(op) {
  if (op.state === "done") return op.destination;
  return op.state === "unrecoverable" ? null : op.source;
}

export function render(root, nav, sessionId, from, message) {
  const fromHistory = from?.page != null;
  const back = el("button", { type: "button" }, fromHistory ? "History" : "Home");
  back.addEventListener("click", () => (fromHistory ? nav.history(from.page) : nav.home()));
  const status = el("p", { className: "status" });
  const content = el("div");
  const actions = el("div", { className: "actions" }, back);
  if (from?.step === "duplicates") {
    // The next step groups what is left, so it scans the Scan Root again first.
    const next = el("button", { type: "button", className: "primary" }, "Continue");
    next.addEventListener("click", () =>
      busy(status, [next, back], `Scanning ${from.scanRoot}…`, () =>
        nav.steps.continueAfterDuplicates(from.scanRoot, (progress) => showStatus(status, describeScanProgress(from.scanRoot, progress))),
      ),
    );
    actions.append(next);
  }
  root.append(actions, content);

  // Kept when the Session is drawn again after an action.
  const view = { group: null, query: "", sort: "size", limit: PAGE_SIZE, selectedId: null };

  /** Draws the Session afresh, with `text` (the last action's result) in the status line. */
  async function load(text) {
    let details;
    try {
      details = await sessionDetails(sessionId);
    } catch (error) {
      content.replaceChildren(el("p", { className: "status error" }, describeError(error)));
      return;
    }
    draw(details);
    showStatus(status, text ?? "");
  }

  /** Runs `call` with every control disabled and `label` in the status line, then draws its result. */
  async function act(label, call, done) {
    const enabled = [...root.querySelectorAll("button, input, select")].filter((c) => !c.disabled);
    for (const control of enabled) control.disabled = true;
    showStatus(status, label);
    let result;
    try {
      result = await call();
    } catch (error) {
      for (const control of enabled) control.disabled = false;
      showStatus(status, describeError(error), true);
      return;
    }
    await load(describeSummary(result, done));
    // The drawn controls are new; this brings back the ones outside them, like Back.
    for (const control of enabled) control.disabled = false;
  }

  function draw({ info, operations }) {
    const rootName = splitPath(info.scanRoot, pathSeparator).name;
    const groups = groupByState(operations);
    // Stay on the group that was listed, while it still has files.
    let group = groups.find((g) => g.group === view.group) ?? groups[0];
    view.group = group?.group ?? null;
    let ticked = new Set();

    const putAll = el("button", { type: "button", className: "primary" }, "Put all back");
    putAll.hidden = !groups.some((g) => g.group === "moved");
    putAll.addEventListener("click", () => act("Putting back…", () => undo(sessionId), "put back"));
    const header = el(
      "header",
      { className: "plan-head" },
      el(
        "div",
        {},
        el("h1", {}, sessionHeading(operations, info.status)),
        el("p", { className: "session-meta" }, `${formatDate(info.startedAt)} · `, el("span", { title: info.scanRoot }, rootName)),
        el("p", { className: "lead" }, "Nothing was deleted. You can put back any file, or all of them, at any time."),
      ),
      el("div", { className: "actions" }, putAll),
    );

    if (!group) {
      content.replaceChildren(header, status, el("p", { className: "muted" }, "This Session has no files."));
      return;
    }

    // Cards: one per group, saying where its files are; they also pick the listed group.
    const line = (...parts) => el("span", { className: "plan-card-line" }, ...parts);
    const muted = (text) => el("span", { className: "muted" }, text);
    const sizeOf = (ops) => formatSize(ops.reduce((sum, op) => sum + op.size, 0));
    function cardText({ group: name, operations: ops }) {
      if (name === "moved") {
        // The folder and its note are one item, so the note wraps with the folder's name.
        const where = ops.every((op) => op.kind === "trash")
          ? el("span", {}, el("strong", {}, holdingName(ops)), muted(`, a folder inside ${rootName}`))
          : "where the Session put them";
        return [line(el("strong", {}, GROUP_COUNTS.moved(ops.length)), muted(" · "), sizeOf(ops), muted(" → "), where)];
      }
      if (name === "putBack") {
        return [line(el("strong", {}, GROUP_COUNTS.putBack(ops.length)), muted(" · "), sizeOf(ops), muted(" → "), "where they were")];
      }
      const [n, ...rest] = GROUP_COUNTS.attention(ops.length).split(" ");
      return [
        line(el("strong", {}, el("span", { className: "session-attention" }, n), ` ${rest.join(" ")}`)),
        el("span", { className: "plan-card-explain" }, "Skipped, failed or stopped part-way. Nothing here was lost."),
      ];
    }
    const cards = groups.map((g) => {
      const pick = el("button", { type: "button", className: "plan-card-pick" }, ...cardText(g));
      const node = el("div", { className: "plan-card session-card" }, pick);
      pick.addEventListener("click", () => {
        if (group === g) return;
        group = g;
        view.group = g.group;
        view.limit = PAGE_SIZE;
        view.selectedId = null;
        ticked = new Set();
        drawCards();
        drawList();
      });
      return { g, node, pick };
    });
    const drawCards = () => {
      for (const { g, node, pick } of cards) {
        node.classList.toggle("current", g === group);
        pick.ariaPressed = String(g === group);
      }
    };

    // The chosen group's files: search and sort, Select all, then one line per file.
    const search = el("input", {
      type: "search",
      className: "search",
      placeholder: "Search file names or folders",
      ariaLabel: "Search",
      value: view.query,
    });
    const sort = el(
      "select",
      { ariaLabel: "Sort" },
      el("option", { value: "size" }, "Biggest first"),
      el("option", { value: "name" }, "Name"),
      el("option", { value: "folder" }, "Folder"),
    );
    sort.value = view.sort;
    for (const [control, key] of [
      [search, "query"],
      [sort, "sort"],
    ]) {
      control.addEventListener(control === search ? "input" : "change", () => {
        view[key] = control.value;
        view.limit = PAGE_SIZE;
        drawList();
        // A file the search now hides is unticked, so the action never takes files out of sight.
        const matching = new Set(files.map((op) => op.id));
        ticked = new Set([...ticked].filter((id) => matching.has(id)));
        drawTicks();
      });
    }
    const selectAll = el("input", { type: "checkbox", ariaLabel: "Select every file" });
    selectAll.addEventListener("change", () => {
      ticked = toggleAll(
        files.map((op) => op.id),
        ticked,
      );
      drawTicks();
    });
    const selectAllLabel = el("label", { className: "plan-include" }, selectAll, "Select all");

    const listHeading = el("h2", {});
    const list = el("ul", { className: "plan-files session-files", ariaLabel: "Files" });
    const more = el("div", { className: "plan-more" });
    const bulk = el("button", { type: "button" });
    bulk.addEventListener("click", () => {
      const action = ACTIONS[group.group];
      const ids = [...ticked];
      act(action.busy, () => action.call(sessionId, ids), action.done);
    });
    const bulkBar = el("div", { className: "session-bulk" }, bulk);
    const pane = el("aside", { className: "plan-pane", ariaLabel: "Selected file" });
    const menu = revealMenu();
    let files = []; // the group's files that match the search
    let rows = []; // [{ op, row, box, target }] for the files on screen; `target` takes focus

    const select = (op) => {
      view.selectedId = op.id;
      for (const r of rows) {
        const current = r.op === op;
        r.row.classList.toggle("selected", current);
        r.row.ariaCurrent = current ? "true" : null;
      }
      drawPane(op);
    };

    function fileRow(op, index) {
      const action = ACTIONS[group.group];
      const box = action ? el("input", { type: "checkbox", ariaLabel: action.tickLabel(fileName(op)) }) : null;
      const folder = folderLabel(op, rootName);
      const row = el(
        "li",
        { className: "plan-file" },
        box ?? "",
        el("span", { className: "plan-file-name", title: fileName(op) }, fileName(op)),
        // Wrapped in left-to-right marks so the right-to-left box keeps the folder's end, not its slashes.
        el(
          "span",
          { className: "plan-file-folder path", id: `session-folder-${index}`, title: [info.scanRoot, ...op.relativePath.slice(0, -1)].join(pathSeparator) },
          `‎${folder}‎`,
        ),
        el("span", { className: "plan-file-size", id: `session-size-${index}` }, formatSize(op.size)),
      );
      // Under Needs attention there is no tick box, so the row itself takes focus.
      const target = box ?? row;
      if (box) {
        // Read after the name when the box gets focus, which is how a row is selected from the keyboard.
        box.setAttribute("aria-describedby", `session-folder-${index} session-size-${index}`);
        box.addEventListener("change", () => {
          ticked = new Set(ticked);
          if (box.checked) ticked.add(op.id);
          else ticked.delete(op.id);
          drawTicks();
        });
      } else row.tabIndex = 0;
      target.addEventListener("focus", () => select(op));
      // WebKit doesn't focus a tick box when it is clicked, so a click selects the row itself.
      row.addEventListener("click", (event) => {
        select(op);
        if (event.target !== box) target.focus();
      });
      row.addEventListener("contextmenu", (event) => {
        event.preventDefault();
        select(op);
        const path = whereNow(op);
        if (path) menu.open([path], row, event);
      });
      return { op, row, box, target };
    }

    // ↑ and ↓ move between files; Space ticks the focused one (it is a tick box).
    list.addEventListener("keydown", (event) => {
      const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
      const at = rows.findIndex((r) => r.target === event.target);
      if (!step || at === -1) return;
      event.preventDefault();
      rows[Math.min(rows.length - 1, Math.max(0, at + step))]?.target.focus();
    });

    function drawList() {
      const action = ACTIONS[group.group];
      listHeading.textContent = `${GROUP_COUNTS[group.group](group.operations.length)}, ${SORT_NAMES[view.sort]}`;
      files = visibleFiles(group.operations, view);
      const { shown } = paging(files.length, view.limit);
      const focused = rows.find((r) => r.target === document.activeElement)?.op.id;
      rows = files.slice(0, shown).map(fileRow);
      list.classList.toggle("no-ticks", !action);
      list.replaceChildren(...(rows.length ? rows.map((r) => r.row) : [el("li", { className: "plan-none" }, "No files match.")]));
      drawMore(more, files.length, shown, (limit) => {
        view.limit = limit;
        drawList();
      });
      selectAllLabel.hidden = !action;
      bulkBar.hidden = !action;
      drawTicks();

      const current = selectedFile(
        rows.map((r) => r.op),
        view.selectedId,
      );
      pane.hidden = !current;
      if (current) select(current);
      rows.find((r) => r.op.id === focused)?.target.focus();
    }

    /** Brings the tick boxes, Select all and the action for the ticked files in step with `ticked`. */
    function drawTicks() {
      for (const { op, box } of rows) if (box) box.checked = ticked.has(op.id);
      const state = tickState(
        files.map((op) => op.id),
        ticked,
      );
      selectAll.checked = files.length > 0 && state === "all";
      selectAll.indeterminate = state === "some";
      selectAll.disabled = files.length === 0;
      const action = ACTIONS[group.group];
      if (!action) return;
      bulk.textContent = ticked.size ? action.bulk(ticked.size) : "Nothing ticked";
      bulk.disabled = ticked.size === 0;
    }

    function drawPane(op) {
      const action = ACTIONS[group.group];
      const path = whereNow(op);
      const buttons = [];
      if (path) {
        const reveal = el("button", { type: "button" }, "Show in folder");
        reveal.addEventListener("click", () => revealInFileManager(path).catch((error) => showStatus(status, describeError(error), true)));
        buttons.push(reveal);
      }
      if (action) {
        const one = el("button", { type: "button" }, action.single);
        one.addEventListener("click", () => act(action.busy, () => action.call(sessionId, [op.id]), action.done));
        buttons.push(one);
      }
      pane.replaceChildren(
        el("h3", { className: "plan-pane-name" }, fileName(op)),
        el("p", { className: "muted" }, formatSize(op.size)),
        el("p", {}, paneSentence(op, rootName, pathSeparator)),
        buttons.length ? el("div", { className: "actions" }, ...buttons) : "",
      );
    }

    content.replaceChildren(
      header,
      status,
      el("div", { className: "plan-cards" }, ...cards.map((c) => c.node)),
      el(
        "div",
        { className: "plan-split" },
        el(
          "section",
          { className: "plan-list" },
          listHeading,
          el("div", { className: "filters" }, search, sort, selectAllLabel),
          list,
          more,
          bulkBar,
          menu.node,
        ),
        pane,
      ),
    );
    drawCards();
    drawList();
  }

  load(message);
}
