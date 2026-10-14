// The Session rows shared by Home and History: date, Scan Root, status, what was done, Undo.
import { describeError, pathSeparator, undo } from "./api.js";
import { el } from "./dom.js";
import { count, formatDate, formatSize, splitPath } from "./format.js";
import { kindIcon, kindLabel } from "./kinds.js";

const STATUS_LABELS = {
  completed: "Completed",
  crashed: "Stopped part-way",
  undone: "Undone",
  partlyUndone: "Partly undone",
};

/** The Session status as a filled pill in its colour. */
export function statusPill(status) {
  return el("span", { className: `pill status-pill status-${status}` }, STATUS_LABELS[status] ?? status);
}

/** What Home and History show before the first Session. */
export function noSessions() {
  return el("p", { className: "empty" }, "No Sessions yet. Once you run a Plan, it shows up here and can be undone.");
}

/**
 * A list of Session rows. Clicking a row opens its Session with `open(id)`; its Undo button
 * undoes the Session and passes the result to `undone(result)`, so the caller can show it.
 */
export function sessionList(sessions, { open, undone }) {
  return el("ul", { className: "sessions" }, ...sessions.map((session) => sessionRow(session, open, undone)));
}

function sessionRow(session, open, undone) {
  const button = el("button", { type: "button" }, "Undo");
  // A partly undone Session can be undone again, to retry what was skipped.
  button.disabled = session.leftToUndo === 0;
  const message = el("p", { className: "status" });
  button.addEventListener("click", async () => {
    button.disabled = true;
    message.classList.remove("error");
    message.textContent = "Undoing…";
    try {
      undone(await undo(session.id));
    } catch (error) {
      message.textContent = describeError(error);
      message.classList.add("error");
      button.disabled = false;
    }
  });

  const kinds = Object.entries(session.kindCounts).map(([kind, n]) =>
    el("span", { className: "kind-count", title: `${kindLabel(kind)}: ${count(n)}` }, kindIcon(kind), count(n)),
  );
  const item = el(
    "li",
    { className: "session", tabIndex: 0, title: `Open this Session\n${session.scanRoot}` },
    el(
      "div",
      { className: "session-row" },
      el("span", { className: "session-date" }, formatDate(session.startedAt)),
      el("span", { className: "session-root" }, splitPath(session.scanRoot, pathSeparator).name),
      statusPill(session.status),
      el("span", { className: "kind-counts" }, ...(kinds.length > 0 ? kinds : [el("span", { className: "muted" }, "Nothing done")])),
      el("span", { className: "session-size" }, session.done > 0 ? formatSize(session.totalSize) : ""),
      button,
    ),
    message,
  );
  // The Undo button does its own thing; anywhere else opens the Session.
  item.addEventListener("click", (event) => {
    if (!event.target.closest("button")) open(session.id);
  });
  item.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && event.target === item) open(session.id);
  });
  return item;
}
