// The Emptied Folders depend on everything the Plan moves, so the backend works them out again
// after each change.
import { buildReviewPlan, pathSeparator, previewReviewPlan, runPlan } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { formatSize, plural } from "../../shared/format.js";
import { describeNothingDone, describeRunProgress, describeWarning } from "../../shared/outcomes.js";
import { refreshMenu } from "../../shared/menus/menus.js";
import { busy, showStatus, showWarning } from "../../shared/status.js";
import { describeChange, nameMissing, planChoices, planDuplicates, totals } from "./review-choices.js";
import { skippedLine } from "./skipped.js";

/** In ms. Batches quick changes, as each answer reads the disk. */
const EMPTIED_DELAY = 300;

/** Blocked Operations are passed too: the Run skips them and lists them under Needs attention. */
const runsOrIsListed = (op) => op.ticked || (Boolean(op.error) && (op.kind === "trash" || op.reason.type === "suggestedFolder"));

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

  /** The menu bar too, as it must be greyed out while a Session runs. */
  function drawHeader() {
    refreshMenu();
    const possible = state.emptied.filter((op) => !op.error);
    emptied.hidden = possible.length === 0;
    emptied.title = possible.map((op) => `${op.relativePath.join("/")}/`).join("\n");
    emptiedText.textContent = `Also trash ${plural(possible.length, "emptied folder")}`;
    const ticked = possible.filter((op) => state.emptiedTicked.has(op.id)).length;
    emptiedBox.checked = ticked > 0;
    if (state.phase === "reading" || state.more === "reading") {
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
    refreshMenu();
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

  /** A Session of its own; files that couldn't go stay in their folder. */
  async function trashFoldersNow(chosen) {
    if (view.running) return;
    const { folders, trashed } = state.trashNow(chosen);
    if (folders.length === 0) return showWarning(status, "Nothing was trashed. No files are listed there.");
    clearTimeout(emptiedTimer);
    const at = Math.min(...chosen.map((choice) => state.choices.indexOf(choice)));
    view.running = true;
    ctx.redraw.pane();
    const summary = await busy(status, [run, cancel], "Trashing files…", async () => {
      const plan = await buildReviewPlan([], folders);
      const approved = plan.operations.filter((op) => op.reason.type === "suggestedFolder" && (op.ticked || Boolean(op.error))).map((op) => op.id);
      return runPlan(approved, (progress) => showStatus(status, describeRunProgress(progress)));
    });
    view.running = false;
    if (summary) {
      trashed(summary.done.map((outcome) => outcome.source));
      const warning = describeWarning(summary, { action: "run", done: "trashed", separator: pathSeparator });
      // A Trash Now that did nothing keeps no Session.
      if (!summary.sessionId) showWarning(status, describeNothingDone(summary, "trashed", pathSeparator));
      else if (warning) showWarning(status, warning);
      else showStatus(status, `${plural(summary.done.length, "file")} trashed · undo from History`);
    }
    if (view.renaming && !state.choices.includes(view.renaming.choice)) view.renaming = null;
    const listed = view.current === "picked" ? [...view.picked] : [view.current];
    if (listed.some((choice) => chosen.includes(choice))) ctx.show(state.choices[Math.min(at, state.choices.length - 1)] ?? "left");
    else ctx.redraw.pane();
  }

  /** Changes can outrun the backend, so only the latest answer counts. */
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

  return { node, drawHeader, status, refreshEmptied, trashFoldersNow };
}
