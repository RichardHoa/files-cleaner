// Summary: what a Session did (done, skipped, failed, with reasons), with Undo.
import { describeError, undo } from "../api.js";
import { el } from "../dom.js";
import { plural } from "../format.js";

export function render(root, nav, { session }) {
  const home = el("button", { type: "button" }, "Home");
  home.addEventListener("click", () => nav.home());
  const openSession = el("button", { type: "button" }, "Open Session");
  openSession.addEventListener("click", () => nav.session(session.sessionId));
  const undoButton = el("button", { type: "button" }, "Undo this Session");
  const status = el("p", { className: "status" });
  const undoResult = el("div");

  undoButton.addEventListener("click", async () => {
    undoButton.disabled = true;
    home.disabled = true;
    status.classList.remove("error");
    status.textContent = "Undoing…";
    try {
      const undone = await undo(session.sessionId);
      status.textContent = "";
      undoResult.replaceChildren(...undoSections(undone));
      // Undo again retries what was skipped, once whatever was in the way has moved.
      undoButton.disabled = undone.skipped.length === 0;
    } catch (error) {
      status.textContent = describeError(error);
      status.classList.add("error");
      undoButton.disabled = false;
    }
    home.disabled = false;
  });

  root.append(
    el("h1", {}, "Summary"),
    el("p", { className: "path" }, `Session ${session.sessionId}`),
    el(
      "p",
      { className: "lead" },
      [`${session.done.length} done`, `${session.skipped.length} skipped`, `${session.failed.length} failed`].join(" · "),
    ),
    el("div", { className: "actions" }, home, openSession, undoButton),
    status,
    undoResult,
    ...outcomeSection("Done", session.done),
    ...outcomeSection("Skipped", session.skipped),
    ...outcomeSection("Failed", session.failed),
  );
}

/** What an Undo did: restored, skipped and unrecoverable items. */
export function undoSections(undone) {
  return [
    el(
      "p",
      { className: "lead" },
      [
        `Undo: ${plural(undone.restored.length, "item")} restored`,
        `${undone.skipped.length} skipped`,
        `${undone.unrecoverable.length} unrecoverable`,
      ].join(" · "),
    ),
    ...outcomeSection("Skipped by Undo", undone.skipped),
    ...outcomeSection("Unrecoverable", undone.unrecoverable),
  ];
}

/** A heading and a list of Operations with their reasons; nothing if there are none. */
function outcomeSection(title, outcomes) {
  if (outcomes.length === 0) return [];
  return [
    el("h2", {}, `${title} (${outcomes.length})`),
    el(
      "ul",
      { className: "outcomes" },
      ...outcomes.map((outcome) =>
        el(
          "li",
          {},
          el("span", { className: "path" }, outcome.source),
          outcome.reason ? el("div", { className: "reason" }, outcome.reason) : "",
        ),
      ),
    ),
  ];
}
