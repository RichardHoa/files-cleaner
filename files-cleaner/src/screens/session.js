// Session: one past Session's Operations, each with its state, and Undo or Redo for each
// Operation or for all of them at once.
import { describeError, pathSeparator, redoOperations, revealInFinder, sessionDetails, undo as undoOperations } from "../api.js";
import { el } from "../dom.js";
import { formatDate, formatSize, plural } from "../format.js";
import { kindPill } from "../kinds.js";
import { describeSummary } from "../outcomes.js";
import { statusPill } from "../session-row.js";
import { busy, showStatus } from "../status.js";

const STATE_LABELS = {
  started: "Stopped part-way",
  done: "Trashed",
  skipped: "Skipped",
  failed: "Failed",
  undone: "Undone",
  unrecoverable: "Unrecoverable",
};

export function render(root, nav, sessionId, from) {
  const back = el("button", { type: "button" }, from ? "History" : "Home");
  back.addEventListener("click", () => (from ? nav.history(from.page) : nav.home()));
  const content = el("div");
  root.append(el("div", { className: "actions" }, back), content);
  show(content, sessionId);
}

/** Draws the Session into `node`, with `message` (the last action's result) above its Operations. */
async function show(node, sessionId, message = null) {
  let details;
  try {
    details = await sessionDetails(sessionId);
  } catch (error) {
    node.replaceChildren(el("p", { className: "status error" }, describeError(error)));
    return;
  }
  const { info, operations } = details;
  const status = el("p", { className: "status" });
  if (message) showStatus(status, message);

  const buttons = [];
  /** Runs `action` with every button disabled, then redraws with its result. */
  const act = (action) => busy(status, buttons, "Working…", async () => show(node, sessionId, await action()));
  const undo = (ids) => act(async () => describeSummary(await undoOperations(sessionId, ids), "restored"));
  const redo = (ids) => act(async () => describeSummary(await redoOperations(sessionId, ids), "trashed again"));

  const idsIn = (state) => operations.filter((op) => op.state === state).map((op) => op.id);
  const undoAll = el("button", { type: "button" }, "Undo all");
  undoAll.disabled = idsIn("done").length === 0;
  undoAll.addEventListener("click", () => undo(idsIn("done")));
  const redoAll = el("button", { type: "button" }, "Redo all");
  redoAll.disabled = idsIn("undone").length === 0;
  redoAll.addEventListener("click", () => redo(idsIn("undone")));
  buttons.push(undoAll, redoAll);

  const relative = (path) => {
    const prefix = info.scanRoot + pathSeparator;
    return path.startsWith(prefix) ? path.slice(prefix.length) : path;
  };

  const rows = operations.map((op) => {
    const actions = [];
    if (op.state === "done" || op.state === "undone") {
      const isDone = op.state === "done";
      const button = el("button", { type: "button" }, isDone ? "Undo" : "Redo");
      button.addEventListener("click", () => (isDone ? undo : redo)([op.id]));
      // Where the item is now: in the Holding Folder while trashed, back in place once undone.
      const reveal = el("button", { type: "button" }, "Show in Finder");
      reveal.addEventListener("click", () =>
        revealInFinder(isDone ? op.destination : op.source).catch((error) => showStatus(status, describeError(error), true)),
      );
      buttons.push(button, reveal);
      actions.push(reveal, button);
    }
    return el(
      "li",
      {},
      el(
        "div",
        { className: "session-row" },
        el("span", { className: "path", title: op.source }, relative(op.source)),
        kindPill(op.kind),
        el("span", { className: `pill state-pill state-${op.state}` }, STATE_LABELS[op.state] ?? op.state),
        el("span", { className: "muted" }, formatSize(op.size)),
        el("span", { className: "row-actions" }, ...actions),
      ),
      op.state === "done" ? el("div", { className: "reason" }, `In ${relative(op.destination)}`) : "",
      op.reason ? el("div", { className: "reason" }, op.reason) : "",
    );
  });

  node.replaceChildren(
    el("h1", {}, `Session ${formatDate(info.startedAt)}`),
    el("p", { className: "path" }, info.scanRoot),
    el(
      "p",
      { className: "lead pill-line" },
      statusPill(info.status),
      [`${plural(info.done, "Operation")} done`, `${info.restored} undone`].join(" · "),
    ),
    el("div", { className: "actions" }, undoAll, redoAll),
    status,
    rows.length > 0 ? el("ul", { className: "outcomes" }, ...rows) : el("p", { className: "status" }, "No Operations."),
  );
}
