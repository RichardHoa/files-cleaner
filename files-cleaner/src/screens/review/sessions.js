// The Review screen's Sessions and header: the header's button, carrying out the rest of the Plan as one
// Session, Cancel, and each part carried out on its own from its pane, with what it did and a link to its
// Session.
import { buildReviewPlan, pathSeparator, runPlan } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { describeNothingDone, describeRunProgress, describeSummary, describeWarning, notDoneSections, warningBlock } from "../../shared/outcomes.js";
import { busy, showStatus, showWarning } from "../../shared/status.js";
import { describeChange, nameMissing, partChoices, planChoices, totals } from "./review-choices.js";

/**
 * Whether a Run is given `op`: it's ticked, or it's a Trash or a Suggested Folder's Move blocked up front, which
 * the Run skips, leaving it in place and listed under Needs attention.
 */
const runsOrIsListed = (op) => op.ticked || (Boolean(op.error) && (op.kind === "trash" || op.reason.type === "suggestedFolder"));

/**
 * The Sessions of the Review screen with context `ctx`: `header`, its heading, button, scope and Cancel,
 * `drawHeader()`, `runPart`, `moveFolder` and `trashPart` carrying out a part on its own, and `outcome` and
 * `trashOutcomes` saying what one did.
 */
export function sessions(ctx) {
  const { nav, state, view, status } = ctx;
  const header = {
    heading: el("h1", {}),
    run: el("button", { type: "button", className: "primary" }),
    // What the header's button covers: every part not yet run, in every folder.
    scope: el("p", { className: "review-run-scope muted" }),
    cancel: el("button", { type: "button" }, "Cancel"),
  };
  const { heading, run, scope, cancel } = header;

  cancel.addEventListener("click", () => {
    if (ctx.reading.cancel()) cancel.disabled = true;
  });

  const drawHeader = () => {
    cancel.textContent = state.ran.size > 0 ? "Done" : "Cancel";
    const change = totals(state.choices, state.left, pathSeparator, [], state.ran);
    if (state.phase === "reading") {
      heading.textContent = ctx.rootName;
      run.textContent = "Waiting for folders…";
      run.disabled = true;
      scope.textContent = "";
      return;
    }
    const emptiedLeft = state.emptied.filter((op) => state.emptiedTicked.has(op.id) && !op.error).length;
    const words = describeChange(change, emptiedLeft, state.ran.size > 0);
    heading.textContent = words.heading;
    run.textContent = words.button;
    scope.textContent = words.scope;
    run.disabled = view.running || change.moved + change.trashed + emptiedLeft === 0 || state.choices.some(nameMissing);
  };

  run.addEventListener("click", async () => {
    ctx.folders.holdEmptied();
    view.running = true;
    const session = await busy(status, [run, cancel], "Planning…", async () => {
      const plan = await buildReviewPlan([], planChoices(state.choices, state.left, state.ran));
      // What already ran is only there to work out the Emptied Folders.
      const approved = plan.operations
        .filter((op) => (op.reason.type === "emptiedFolder" ? state.emptiedTicked.has(op.id) && !op.error : runsOrIsListed(op) && !state.ran.has(op.source)))
        .map((op) => op.id);
      return runPlan(approved, (progress) => showStatus(status, describeRunProgress(progress)));
    });
    view.running = false;
    if (session?.sessionId) return nav.session(session.sessionId, null, { summary: session, action: "run", done: "moved" });
    // A Run that did nothing keeps no Session: the review stays, saying why.
    if (session) showWarning(status, describeNothingDone(session, "moved", pathSeparator));
    drawHeader();
  });

  /**
   * Carries out one part of the Plan, the copies in `duplicates` and the Suggested Folders in `choices` as
   * `buildReviewPlan` takes them, as a Session of its own, saying `label` meanwhile and what wasn't `done`
   * ("moved" or "trashed") if nothing was. Resolves to what it did, or null if it made no Session.
   */
  async function runPart(button, { duplicates = [], choices = [] }, label, done) {
    view.running = true;
    const summary = await busy(status, [button, run, cancel], label, async () => {
      const plan = await buildReviewPlan(duplicates, choices);
      const approved = plan.operations.filter((op) => op.reason.type !== "emptiedFolder" && runsOrIsListed(op)).map((op) => op.id);
      return runPlan(approved, (progress) => showStatus(status, describeRunProgress(progress)));
    });
    view.running = false;
    if (!summary?.sessionId) {
      if (summary) showWarning(status, describeNothingDone(summary, done, pathSeparator));
      return null;
    }
    showStatus(status, "");
    return summary;
  }

  /** Moves the files going into the Suggested Folder `choice`, which is then finished: only its trashes can still run. */
  async function moveFolder(choice, button) {
    const summary = await runPart(button, { choices: partChoices(choice, "move", state.ran) }, "Moving the files…", "moved");
    if (summary) state.ranMoves(choice, summary);
    ctx.redraw.pane();
  }

  /** Trashes the files of `choices`, a Suggested Folder's or those left where they are, keeping what it did under `key`. */
  async function trashPart(key, choices, button) {
    const summary = await runPart(button, { choices }, "Trashing the files…", "trashed");
    if (summary) state.ranTrashes(key, summary);
    ctx.redraw.pane();
  }

  /**
   * What a part carried out on its own did, as `summary`: the warning if it left any, its counts with what was
   * `done` ("moved" or "trashed"), the files it left with why, then a link to its Session's screen, where
   * what it did can be checked or undone.
   */
  function outcome(summary, done) {
    const sections = notDoneSections(summary);
    const text = describeWarning(summary, { action: "run", done, separator: pathSeparator });
    const link = el("button", { type: "button", className: "link" }, "Open this Session");
    link.addEventListener("click", () => nav.session(summary.sessionId));
    return [
      text ? warningBlock(text, () => sections[0]?.scrollIntoView({ behavior: "smooth", block: "start" })) : "",
      el("p", {}, describeSummary(summary, done)),
      ...sections,
      el("p", {}, link),
    ];
  }

  /** What each Session of the trashes kept under `key`, a Suggested Folder or "left", did. */
  const trashOutcomes = (key) => (state.trashesDone.get(key) ?? []).flatMap((summary) => outcome(summary, "trashed"));

  return { header, drawHeader, runPart, moveFolder, trashPart, outcome, trashOutcomes };
}
