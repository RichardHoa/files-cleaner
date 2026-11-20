// The Review screen's header and its one Session: the Scan Root's name, what was scanned, the button carrying
// out the whole Plan as one Session, Cancel, and "Also trash N emptied folders". The Emptied Folders depend on
// everything the Plan moves, so they are worked out by the backend again a moment after each change. What the
// button is doing, or why it did nothing, shows under it.
import { buildReviewPlan, pathSeparator, previewReviewPlan, runPlan } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { formatSize, plural } from "../../shared/format.js";
import { describeNothingDone, describeRunProgress } from "../../shared/outcomes.js";
import { busy, showStatus, showWarning } from "../../shared/status.js";
import { describeChange, nameMissing, planChoices, planDuplicates, totals } from "./review-choices.js";
import { skippedLine } from "./skipped.js";

/** How long the Emptied Folders wait after a change before they're worked out again, in ms. */
const EMPTIED_DELAY = 300;

/**
 * Whether a Run is given `op`: it's ticked, or it's a Trash or a Suggested Folder's Move blocked up front, which
 * the Run skips, leaving it in place and listed under Needs attention.
 */
const runsOrIsListed = (op) => op.ticked || (Boolean(op.error) && (op.kind === "trash" || op.reason.type === "suggestedFolder"));

/**
 * The header of the Review screen with context `ctx`, for the scan `result`: `node`, `drawHeader()`,
 * `status`, the line under the button, and `refreshEmptied()` working out the Emptied Folders again a moment
 * later, unless the button is clicked first.
 */
export function sessions(ctx, result) {
  const { nav, state, view } = ctx;
  const run = el("button", { type: "button", className: "primary" });
  const cancel = el("button", { type: "button" }, "Cancel");
  const status = el("p", { className: "status review-run-status" });
  const emptiedBox = el("input", { type: "checkbox" });
  const emptiedText = el("span");
  const emptied = el("label", { className: "review-emptied" }, emptiedBox, emptiedText);
  const skipped = skippedLine(result);
  const totalSize = result.entries.reduce((sum, entry) => sum + entry.size, 0);

  const node = el(
    "header",
    { className: "plan-head" },
    el(
      "div",
      {},
      el("h1", { title: result.root }, ctx.rootName),
      el(
        "p",
        { className: "lead" },
        `${plural(result.entries.length, "item")} · ${formatSize(totalSize)}`,
        ...(skipped ? [" · ", skippedPopover(skipped)] : []),
      ),
    ),
    el("div", { className: "review-run" }, el("div", { className: "actions" }, emptied, cancel, run), status),
  );

  /** "Some files skipped", opening its detail under it on a click; a click anywhere else, or Esc, closes it. */
  function skippedPopover({ label, detail: text }) {
    const toggle = el("button", { type: "button", className: "skipped-line", ariaExpanded: "false" }, label);
    const detail = el("div", { className: "row-menu skipped-detail", hidden: true }, text);
    const wrap = el("span", { className: "skipped" }, toggle, detail);
    const show = (open) => {
      detail.hidden = !open;
      toggle.ariaExpanded = String(open);
    };
    toggle.addEventListener("click", () => show(detail.hidden));
    document.addEventListener("click", (event) => wrap.contains(event.target) || show(false));
    wrap.addEventListener("keydown", (event) => {
      if (event.key !== "Escape" || detail.hidden) return;
      show(false);
      toggle.focus();
    });
    return wrap;
  }

  cancel.addEventListener("click", () => {
    if (ctx.reading.cancel()) cancel.disabled = true;
  });
  emptiedBox.addEventListener("change", () => {
    state.tickEmptied(emptiedBox.checked);
    drawHeader();
  });

  /** The emptied-folders checkbox, and the button saying what it will do, disabled while there's nothing to do, a folder has no name, or the files are still read. */
  function drawHeader() {
    const possible = state.emptied.filter((op) => !op.error);
    emptied.hidden = possible.length === 0;
    emptied.title = possible.map((op) => `${op.relativePath.join("/")}/`).join("\n");
    emptiedText.textContent = `Also trash ${plural(possible.length, "emptied folder")}`;
    const ticked = possible.filter((op) => state.emptiedTicked.has(op.id)).length;
    emptiedBox.checked = ticked > 0;
    if (state.phase === "reading") {
      run.textContent = "Waiting for folders…";
      run.disabled = true;
      return;
    }
    const change = totals(state.choices, state.left, pathSeparator, state.duplicates);
    run.textContent = describeChange(change, ticked);
    run.disabled = view.running || change.moved + change.trashed + ticked === 0 || state.choices.some(nameMissing);
  }

  run.addEventListener("click", async () => {
    clearTimeout(emptiedTimer);
    view.running = true;
    const session = await busy(status, [run, cancel], "Planning…", async () => {
      const plan = await buildReviewPlan(planDuplicates(state.duplicates), planChoices(state.choices, state.left));
      const approved = plan.operations
        .filter((op) => (op.reason.type === "emptiedFolder" ? state.emptiedTicked.has(op.id) && !op.error : runsOrIsListed(op)))
        .map((op) => op.id);
      return runPlan(approved, (progress) => showStatus(status, describeRunProgress(progress)));
    });
    view.running = false;
    if (session?.sessionId) return nav.session(session.sessionId, null, { summary: session, action: "run", done: "moved" });
    // A Run that did nothing keeps no Session: the review stays, saying why.
    if (session) showWarning(status, describeNothingDone(session, "moved", pathSeparator));
    drawHeader();
  });

  /** Works out the Emptied Folders again a moment after a change, with the duplicates; only the latest answer counts. */
  let emptiedTimer = 0;
  let emptiedAsked = 0;
  const refreshEmptied = () => {
    clearTimeout(emptiedTimer);
    emptiedTimer = setTimeout(async () => {
      if (!node.isConnected) return;
      const asked = ++emptiedAsked;
      try {
        const plan = await previewReviewPlan(planDuplicates(state.duplicates), planChoices(state.choices, state.left));
        if (asked !== emptiedAsked || !node.isConnected) return;
        state.setEmptied(plan.operations.filter((op) => op.reason.type === "emptiedFolder"));
        drawHeader();
      } catch {
        // A name that can't be a folder name: the Emptied Folders stay as they were until it's fixed.
      }
    }, EMPTIED_DELAY);
  };

  return { node, drawHeader, status, refreshEmptied };
}
