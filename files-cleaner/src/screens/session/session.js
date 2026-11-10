// Session: what one past Session did and where its files are now. One card per group (Moved,
// Trashed, Put back, Needs attention) picks which files are listed; the selected file is shown beside
// the list. Files can be put back, moved again, or tried again if skipped or failed, all at once,
// ticked, or one at a time, and a file not in the System Trash can be opened, by double-clicking it or
// from its right-click menu.
import { describeError, openSessionFile, pathSeparator, redoOperations, retryOperations, revealInFileManager, sessionDetails, undo } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { drawMore, revealMenu } from "../../shared/plan/file-list.js";
import { count, formatDate, formatSize, plural, splitPath } from "../../shared/format.js";
import { describeSummary } from "../../shared/outcomes.js";
import { tickState, toggleAll } from "../../shared/plan/plan-selection.js";
import { PAGE_SIZE, fileName, folderLabel, paging, selectedFile, visibleFiles } from "../../shared/plan/plan-view.js";
import { actionFor, groupByState, paneSentence, sessionHeading, sessionLead, whereNow } from "./session-view.js";
import { showStatus } from "../../shared/status.js";

/** "3 moved": how many files a group has, as its card and list heading say it. */
const GROUP_COUNTS = {
  moved: (n) => `${count(n)} moved`,
  trashed: (n) => `${count(n)} trashed`,
  putBack: (n) => `${count(n)} put back`,
  attention: (n) => `${count(n)} ${n === 1 ? "needs" : "need"} attention`,
};

/** Each action `actionFor` names, and how it is worded. */
const ACTIONS = {
  undo: {
    bulk: (n) => `Put back ${plural(n, "file")}`,
    single: "Put back",
    tickLabel: (name) => `Put back ${name}`,
    busy: "Putting back…",
    done: "put back",
    call: (sessionId, ids) => undo(sessionId, ids),
  },
  redo: {
    bulk: (n) => `Move ${plural(n, "file")} again`,
    single: "Move again",
    tickLabel: (name) => `Move ${name} again`,
    busy: "Moving…",
    done: "moved again",
    call: (sessionId, ids) => redoOperations(sessionId, ids),
  },
  retry: {
    bulk: (n) => `Try ${plural(n, "file")} again`,
    single: "Try again",
    tickLabel: (name) => `Try ${name} again`,
    busy: "Trying again…",
    done: "done",
    call: (sessionId, ids) => retryOperations(sessionId, ids),
  },
};

/** The action for `op`'s file, if it has one. */
const actionOf = (op) => ACTIONS[actionFor(op)] ?? null;

/** The action for a group's files: every file in a group that has one has the same. Under Needs attention, a file stopped part-way has none. */
const groupAction = (group) => group.operations.map(actionOf).find(Boolean) ?? null;

const SORT_NAMES = { size: "biggest first", name: "by name", folder: "by folder" };

export function render(root, nav, sessionId, from, message) {
  const fromHistory = from?.page != null;
  const back = el("button", { type: "button" }, fromHistory ? "History" : "Home");
  back.addEventListener("click", () => (fromHistory ? nav.history(from.page) : nav.home()));
  const status = el("p", { className: "status" });
  const content = el("div");
  root.append(el("div", { className: "actions" }, back), content);

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
    putAll.hidden = !groups.some((g) => g.group === "moved" || g.group === "trashed");
    putAll.addEventListener("click", () => act("Putting back…", () => undo(sessionId), "put back"));
    const header = el(
      "header",
      { className: "plan-head" },
      el(
        "div",
        {},
        el("h1", {}, sessionHeading(operations, info.status)),
        el("p", { className: "session-meta" }, `${formatDate(info.startedAt)} · `, el("span", { title: info.scanRoot }, rootName)),
        el("p", { className: "lead session-lead" }, sessionLead(operations, pathSeparator)),
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
      if (name !== "attention") return [line(el("strong", {}, GROUP_COUNTS[name](ops.length)), muted(" · "), sizeOf(ops))];
      const [n, ...rest] = GROUP_COUNTS.attention(ops.length).split(" ");
      return [
        line(el("strong", {}, el("span", { className: "session-attention" }, n), ` ${rest.join(" ")}`)),
        el("span", { className: "plan-card-explain" }, "Skipped, failed, stopped part-way or no longer recoverable. Each file says why."),
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
      ticked = toggleAll(tickable(), ticked);
      drawTicks();
    });
    const selectAllLabel = el("label", { className: "plan-include" }, selectAll, "Select all");

    const listHeading = el("h2", {});
    const list = el("ul", { className: "plan-files session-files", ariaLabel: "Files" });
    const more = el("div", { className: "plan-more" });
    const bulk = el("button", { type: "button" });
    bulk.addEventListener("click", () => {
      const action = groupAction(group);
      const ids = [...ticked];
      act(action.busy, () => action.call(sessionId, ids), action.done);
    });
    const bulkBar = el("div", { className: "session-bulk" }, bulk);
    const pane = el("aside", { className: "plan-pane", ariaLabel: "Selected file" });
    const menu = revealMenu();
    let files = []; // the group's files that match the search
    const tickable = () => files.filter(actionOf).map((op) => op.id);
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
      const action = actionOf(op);
      const box = action ? el("input", { type: "checkbox", ariaLabel: action.tickLabel(fileName(op)) }) : null;
      const folder = folderLabel(op, rootName);
      const row = el(
        "li",
        { className: "plan-file" },
        // A file with no tick box in a list that has them keeps its place in the box's column.
        box ?? (groupAction(group) ? el("span") : ""),
        el("span", { className: "plan-file-name", title: fileName(op) }, fileName(op)),
        // Wrapped in left-to-right marks so the right-to-left box keeps the folder's end, not its slashes.
        el(
          "span",
          { className: "plan-file-folder path", id: `session-folder-${index}`, title: [info.scanRoot, ...op.relativePath.slice(0, -1)].join(pathSeparator) },
          `‎${folder}‎`,
        ),
        el("span", { className: "plan-file-size", id: `session-size-${index}` }, formatSize(op.size)),
      );
      // A file with no tick box, such as one stopped part-way, takes focus itself.
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
      const path = whereNow(op);
      const openIt = () => openSessionFile(sessionId, op.id).catch((error) => showStatus(status, describeError(error), true));
      row.addEventListener("dblclick", (event) => {
        if (path && event.target !== box) openIt();
      });
      row.addEventListener("contextmenu", (event) => {
        event.preventDefault();
        select(op);
        if (path) menu.open([path], row, event, [], openIt);
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
      const action = groupAction(group);
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
      const ids = tickable();
      const state = tickState(ids, ticked);
      selectAll.checked = ids.length > 0 && state === "all";
      selectAll.indeterminate = state === "some";
      selectAll.disabled = ids.length === 0;
      const action = groupAction(group);
      if (!action) return;
      bulk.textContent = ticked.size ? action.bulk(ticked.size) : "Nothing ticked";
      bulk.disabled = ticked.size === 0;
    }

    function drawPane(op) {
      const action = actionOf(op);
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
