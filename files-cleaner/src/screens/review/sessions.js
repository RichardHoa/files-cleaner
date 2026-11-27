// The Emptied Folders depend on everything the Plan moves, so the backend works them out again
// after each change.
import { buildReviewPlan, pathSeparator, previewReviewPlan, runPlan } from "../../shared/api.js";
import { busy, el, showStatus, showWarning } from "../../shared/dom.js";
import { count, formatSize, plural } from "../../shared/format.js";
import { refreshMenu } from "../../shared/menus.js";
import { NOT_STARTED, describeNothingDone, describeWarning, notPutBack, runCancel, showCancelled, showRunProgress } from "../../shared/outcomes.js";
import { describeChange, nameMissing, planChoices, planDuplicates, totals } from "./review-choices.js";

/** In ms. Batches quick changes, as each answer reads the disk. */
const EMPTIED_DELAY = 300;

/** Blocked Operations are passed too: the Run skips them and lists them under Needs attention. */
const runsOrIsListed = (op) => op.ticked || (Boolean(op.error) && (op.kind === "trash" || op.reason.type === "suggestedFolder"));

export function sessions(ctx, result) {
  const { nav, state, view } = ctx;
  const run = el("button", { type: "button", className: "primary" });
  const cancel = el("button", { type: "button" }, "Cancel");
  const status = el("p", { className: "status review-run-status" });
  // A folder's Trash Now starts from a menu, so its Cancel has no button to take the place of.
  const trashNowCancel = el("button", { type: "button", className: "small", hidden: true });
  const sessionCancel = runCancel("Puts back everything done so far");
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
    el("div", { className: "review-run" }, el("div", { className: "actions" }, emptied, cancel, run), el("div", { className: "review-run-progress" }, status, trashNowCancel)),
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
    run.classList.toggle("primary", !sessionCancel.shownOn(run));
    run.title = "";
    if (sessionCancel.shownOn(run)) return sessionCancel.draw();
    if (state.phase === "reading" || state.more === "reading") {
      run.textContent = "Waiting for folders…";
      run.disabled = true;
      return;
    }
    const change = totals(state.choices, state.left, pathSeparator, state.duplicates);
    run.textContent = describeChange(change, ticked);
    run.disabled = view.running || change.moved + change.trashed + ticked === 0 || state.choices.some(nameMissing);
  }

  run.addEventListener("click", () => (sessionCancel.shownOn(run) ? sessionCancel.cancel() : runApproved()));
  trashNowCancel.addEventListener("click", () => sessionCancel.cancel());

  async function runApproved() {
    clearTimeout(emptiedTimer);
    view.running = true;
    sessionCancel.start(run);
    drawHeader();
    const session = await busy(status, [cancel], "Planning…", async () => {
      const plan = await buildReviewPlan(planDuplicates(state.duplicates), planChoices(state.choices, state.left));
      if (sessionCancel.asked) return NOT_STARTED;
      const approved = plan.operations
        .filter((op) => (op.reason.type === "emptiedFolder" ? state.emptiedTicked.has(op.id) && !op.error : runsOrIsListed(op)))
        .map((op) => op.id);
      return runPlan(approved, showRunProgress(status, { verb: "Moving", of: (n) => plural(n, "item") }));
    });
    view.running = false;
    const tooLate = sessionCancel.finish();
    if (session?.cancelled) {
      const notBack = notPutBack(session);
      state.gone(notBack.map((outcome) => outcome.source));
      showCancelled(status, notBack, { every: "everything", of: (n) => plural(n, "item") });
      refreshEmptied();
      ctx.redraw.pane();
    } else if (session?.sessionId) return nav.session(session.sessionId, null, { summary: session, action: "run", done: "moved", tooLate });
    // A Run that did nothing keeps no Session: the review stays, saying why.
    else if (session) showWarning(status, describeNothingDone(session, "moved", pathSeparator));
    drawHeader();
  }

  /** A Session of its own; files that couldn't go stay in their folder. */
  async function trashFoldersNow(chosen) {
    if (view.running) return;
    const { folders, trashed } = state.trashNow(chosen);
    if (folders.length === 0) return showWarning(status, "Nothing was trashed. No files are listed there.");
    clearTimeout(emptiedTimer);
    const at = Math.min(...chosen.map((choice) => state.choices.indexOf(choice)));
    view.running = true;
    trashNowCancel.hidden = false;
    sessionCancel.start(trashNowCancel);
    ctx.redraw.pane();
    const summary = await busy(status, [run, cancel], "Trashing files…", async () => {
      const plan = await buildReviewPlan([], folders);
      if (sessionCancel.asked) return NOT_STARTED;
      const approved = plan.operations.filter((op) => op.reason.type === "suggestedFolder" && (op.ticked || Boolean(op.error))).map((op) => op.id);
      return runPlan(approved, showRunProgress(status, { verb: "Trashing", of: (n) => plural(n, "file") }));
    });
    view.running = false;
    const tooLate = sessionCancel.finish();
    trashNowCancel.hidden = true;
    drawHeader();
    if (summary?.cancelled) {
      const notBack = notPutBack(summary);
      trashed(notBack.map((outcome) => outcome.source));
      showCancelled(status, notBack, { every: "every file", of: (n) => plural(n, "file") });
    } else if (summary) {
      trashed(summary.done.map((outcome) => outcome.source));
      const warning = describeWarning(summary, { action: "run", done: "trashed", separator: pathSeparator });
      // A Trash Now that did nothing keeps no Session.
      if (!summary.sessionId) showWarning(status, describeNothingDone(summary, "trashed", pathSeparator));
      else if (warning) showWarning(status, warning);
      else showStatus(status, `${tooLate ? "Too late to cancel · " : ""}${plural(summary.done.length, "file")} trashed · undo from History`);
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

// One quiet line: what was left out matters only to some.

/** True of every scan, so not worth showing the line for. */
const ALWAYS = [
  "Only files directly in the folder are sorted into folders. Files deeper down are checked for duplicates.",
  "Hidden files and symlinks are left alone.",
];

export function skippedLine({ skipped, noExtension, unreadable }) {
  const lines = [
    noExtension > 0 && `Files without an extension: ${count(noExtension)}. They're checked for duplicates but not sorted.`,
    skipped > 0 && `Protected folders: ${count(skipped)}. Folders the system and your apps need, and your Never-Touch Paths, are left alone.`,
    unreadable.length > 0 && `Unreadable: ${count(unreadable.length)}\n${unreadable.join("\n")}`,
  ].filter(Boolean);
  return lines.length > 0 ? { label: "Some files skipped", detail: [...lines, "", ...ALWAYS].join("\n") } : null;
}
