import { describeError, listSessions, pathSeparator, undo } from "../../shared/api.js";
import { busy, el } from "../../shared/dom.js";
import { count, formatDate, formatSize, plural, splitPath } from "../../shared/format.js";
import { undoSections } from "../../shared/outcomes.js";

const PER_PAGE = 10;

export function render(root, nav, page) {
  const home = el("button", { type: "button" }, "Home");
  home.addEventListener("click", () => nav.home());
  const total = el("p", { className: "lead" });
  const content = el("div");
  root.append(el("div", { className: "actions" }, home), el("h1", {}, "History"), total, content);
  show(content, total, nav, page);
}

async function show(node, totalLine, nav, page, undoResult = null) {
  let result;
  try {
    result = await listSessions((page - 1) * PER_PAGE, PER_PAGE);
  } catch (error) {
    node.replaceChildren(el("p", { className: "status error" }, describeError(error)));
    return;
  }
  const pages = pageCount(result.total, PER_PAGE);
  // Sessions can't be deleted today, but a page past the end shows the last one rather than nothing.
  if (page > pages && pages > 0) return show(node, totalLine, nav, pages, undoResult);

  totalLine.textContent = result.total === 0 ? "" : `${plural(result.total, "Session")} · page ${count(page)} of ${count(pages)}`;
  const list =
    result.sessions.length > 0
      ? sessionList(result.sessions, {
          open: (id) => nav.session(id, { page }),
          undone: (undone) => show(node, totalLine, nav, page, undone),
        })
      : noSessions();
  node.replaceChildren(...(undoResult ? undoSections(undoResult, pathSeparator) : []), list, pager(page, pages, (to) => nav.history(to)));
}

function pager(page, pages, go) {
  if (pages <= 1) return "";
  const link = (label, to, ariaLabel) => {
    const button = el("button", { type: "button", className: "page-link" }, label);
    if (ariaLabel) button.ariaLabel = ariaLabel;
    button.disabled = to < 1 || to > pages;
    button.addEventListener("click", () => go(to));
    return button;
  };
  const numbers = pageNumbers(page, pages).map((n) => {
    if (n === null) return el("span", { className: "page-gap" }, "…");
    const button = link(count(n), n, `Page ${n}`);
    if (n === page) {
      button.classList.add("current");
      button.ariaCurrent = "page";
    }
    return button;
  });
  return el("nav", { className: "pager", ariaLabel: "Pages" }, link("‹ Previous", page - 1), ...numbers, link("Next ›", page + 1));
}

// Never shows all page links once there are many.

export function pageCount(total, perPage) {
  return Math.ceil(total / perPage);
}

/** `null` marks a gap. A gap of one page shows the page, as "…" would take the same room. */
export function pageNumbers(current, count) {
  const wanted = [1, current - 1, current, current + 1, count].filter((n) => n >= 1 && n <= count);
  const pages = [...new Set(wanted)].sort((a, b) => a - b);
  const shown = [];
  for (const page of pages) {
    const previous = shown.at(-1);
    if (previous === page - 2) shown.push(page - 1);
    else if (previous != null && previous < page - 2) shown.push(null);
    shown.push(page);
  }
  return shown;
}

const STATUS_LABELS = {
  completed: "Completed",
  crashed: "Stopped part-way",
  undone: "Undone",
  partlyUndone: "Partly undone",
};

export function statusPill(status) {
  return el("span", { className: `pill status-pill status-${status}` }, STATUS_LABELS[status] ?? status);
}

export function noSessions() {
  return el("p", { className: "empty" }, "No Sessions yet");
}

export function sessionList(sessions, { open, undone }) {
  return el("ul", { className: "sessions" }, ...sessions.map((session) => sessionRow(session, open, undone)));
}

function sessionRow(session, open, undone) {
  const button = el("button", { type: "button" }, "Undo");
  // A partly undone Session can be undone again, to retry what was skipped.
  button.disabled = session.leftToUndo === 0;
  const message = el("p", { className: "status" });
  button.addEventListener("click", async () => {
    const result = await busy(message, [button], "Undoing…", () => undo(session.id));
    if (result) undone(result);
  });

  // Folders created along the way are left out: what matters is where the files went.
  const { move = 0, trash = 0 } = session.kindCounts;
  const done = [move && `${count(move)} moved`, trash && `${count(trash)} trashed`].filter(Boolean).join(" · ");
  const item = el(
    "li",
    { className: "session", tabIndex: 0, title: `Open this Session\n${session.scanRoot}` },
    el(
      "div",
      { className: "session-row" },
      el("span", { className: "session-date" }, formatDate(session.startedAt)),
      el("span", { className: "session-root" }, splitPath(session.scanRoot, pathSeparator).name),
      statusPill(session.status),
      el("span", { className: `kind-counts${done ? "" : " muted"}` }, done || "Nothing done"),
      el("span", { className: "session-size" }, session.done > 0 ? formatSize(session.totalSize) : ""),
      button,
    ),
    message,
  );
  // The Undo button does its own thing.
  item.addEventListener("click", (event) => {
    if (!event.target.closest("button")) open(session.id);
  });
  item.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && event.target === item) open(session.id);
  });
  return item;
}
