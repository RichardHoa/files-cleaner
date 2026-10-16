// Summary: what a Session did (done, skipped, failed, with reasons), with Undo.
import { undo } from "../api.js";
import { el } from "../dom.js";
import { describeSummary, outcomeSections, undoSections } from "../outcomes.js";
import { busy, showStatus } from "../status.js";

export function render(root, nav, { session }) {
  const home = el("button", { type: "button" }, "Home");
  home.addEventListener("click", () => nav.home());
  const openSession = el("button", { type: "button" }, "Open Session");
  openSession.addEventListener("click", () => nav.session(session.sessionId));
  const undoButton = el("button", { type: "button" }, "Undo this Session");
  const status = el("p", { className: "status" });
  const undoResult = el("div");

  undoButton.addEventListener("click", async () => {
    const undone = await busy(status, [undoButton, home], "Undoing…", () => undo(session.sessionId));
    if (!undone) return;
    showStatus(status, "");
    undoResult.replaceChildren(...undoSections(undone));
    // Undo again retries what was skipped, once whatever was in the way has moved.
    undoButton.disabled = undone.skipped.length === 0;
  });

  root.append(
    el("h1", {}, "Summary"),
    el("p", { className: "path" }, `Session ${session.sessionId}`),
    el("p", { className: "lead" }, describeSummary(session, "trashed")),
    el("div", { className: "actions" }, home, openSession, undoButton),
    status,
    undoResult,
    ...outcomeSections(session, { done: "Done", skipped: "Skipped", failed: "Failed" }),
  );
}
