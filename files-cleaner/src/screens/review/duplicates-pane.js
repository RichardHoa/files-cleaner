// Trashing copies is a Session of its own, so the user can free space without waiting for the
// Suggested Folders.
import { buildReviewPlan, pathSeparator, runPlan } from "../../shared/api.js";
import { busy, drawMore, el, icon, showStatus, showWarning } from "../../shared/dom.js";
import { formatDate, formatSize, trashPlace } from "../../shared/format.js";
import { withShortcut } from "../../shared/menus.js";
import { NOT_STARTED, describeNothingDone, describeWarning, notPutBack, runCancel, showCancelled, showRunProgress } from "../../shared/outcomes.js";
import { copyFolder, filedCopy, planDuplicates, trashedCopies, trashedCopiesIn } from "./review-choices.js";
import { copiesText, fileName } from "./selection.js";

export function duplicatesPane(ctx) {
  const { state, view, pane } = ctx;
  const trashNow = el("button", { type: "button", className: "small" });
  // Outside the redrawn part, so the result survives redraws.
  const status = el("p", { className: "status duplicates-trash-status" });
  const cancel = runCancel("Puts back every copy trashed so far");
  trashNow.addEventListener("click", () => (cancel.shownOn(trashNow) ? cancel.cancel() : trashCopiesNow()));

  function panel() {
    if (state.duplicates.length === 0) return [el("h2", {}, "Duplicates"), status, el("p", { className: "note" }, "No duplicates")];
    const trashed = trashedCopiesIn(state.duplicates);
    const trashedSize = state.duplicates.reduce((sum, duplicate) => sum + duplicate.group.size * trashedCopies(duplicate), 0);
    const shown = Math.min(state.duplicates.length, view.duplicatesShown);
    const more = el("div", { className: "plan-more" });
    drawMore(more, state.duplicates.length, shown, (limit) => {
      view.duplicatesShown = limit;
      ctx.redraw.pane();
    });
    return [
      el("h2", {}, "Duplicates"),
      el(
        "p",
        { className: "pane-note muted" },
        `Files with exactly the same contents, wherever they are. Ticked copies are kept; unticked copies go to the ${trashPlace(pathSeparator)}, where you can restore them.`,
      ),
      trashNowRow(trashed, trashedSize),
      status,
      el(
        "div",
        { className: "duplicate-columns muted", ariaHidden: "true" },
        el("span", {}, "Keep"),
        el("span", { title: "The folder this copy is in now." }, "Folder"),
        el("span", {}, "Modified"),
      ),
      el("ul", { className: "duplicate-groups", ariaLabel: "Duplicate Groups" }, ...state.duplicates.slice(0, shown).map(duplicateGroup)),
      more,
    ];
  }

  function trashNowRow(trashed, trashedSize) {
    const trash = trashPlace(pathSeparator);
    if (cancel.shownOn(trashNow)) cancel.draw();
    else {
      trashNow.replaceChildren(icon("trash"), el("span", {}, trashed ? `Trash ${copiesText(trashed)} now` : "No copies to trash"));
      trashNow.disabled = trashed === 0 || view.running;
      trashNow.title = withShortcut(`Moves every unticked copy to the ${trash} now. Suggested Folders aren't touched. Undo it from History.`, "trashNow");
    }
    return el(
      "div",
      { className: "duplicates-trash-now" },
      trashNow,
      trashed ? el("span", { className: "muted" }, `Unticked copies only · ${formatSize(trashedSize)} → ${trash}`) : "",
    );
  }

  /** Copies that couldn't go stay marked Trash, for the rest of the Plan. */
  async function trashCopiesNow() {
    if (view.running) return;
    view.running = true;
    cancel.start(trashNow);
    ctx.redraw.header();
    const summary = await busy(status, [], "Trashing copies…", async () => {
      const plan = await buildReviewPlan(planDuplicates(state.duplicates), []);
      if (cancel.asked) return NOT_STARTED;
      const approved = plan.operations.filter((op) => op.reason.type === "duplicate" && (op.ticked || Boolean(op.error))).map((op) => op.id);
      return runPlan(approved, showRunProgress(status, { verb: "Trashing", of: copiesText }));
    });
    view.running = false;
    const tooLate = cancel.finish();
    if (summary?.cancelled) {
      const notBack = notPutBack(summary);
      showCancelled(status, notBack, { every: "every copy", of: copiesText });
      state.copiesTrashed(notBack.map((outcome) => outcome.source));
    } else if (summary) {
      state.copiesTrashed(summary.done.map((outcome) => outcome.source));
      const warning = describeWarning(summary, { action: "run", done: "trashed", separator: pathSeparator });
      // A Run that did nothing keeps no Session.
      if (!summary.sessionId) showWarning(status, describeNothingDone(summary, "trashed", pathSeparator));
      else if (warning) showWarning(status, warning);
      else showStatus(status, `${tooLate ? "Too late to cancel · " : ""}${copiesText(summary.done.length)} trashed · undo from History`);
    }
    ctx.redraw.pane();
  }

  function duplicateGroup(duplicate) {
    const { group } = duplicate;
    const filed = filedCopy(duplicate);
    const name = fileName(filed ?? group.copies[0].path);
    return el(
      "li",
      { className: "duplicate-group" },
      el(
        "div",
        { className: "duplicate-group-head" },
        el("span", { className: "duplicate-group-name", title: name }, name),
        filed ? "" : el("span", { className: "duplicate-group-tally warning" }, "No copy kept"),
      ),
      el("ul", { className: "duplicate-copies" }, ...group.copies.map((copy) => duplicateCopy(duplicate, copy))),
    );
  }

  function duplicateCopy(duplicate, copy) {
    const kept = duplicate.copies.get(copy.path) === "keep";
    // Only once it really sits in a Suggested Folder, or the tag would say what won't happen.
    const filed = copy.path === filedCopy(duplicate) && !state.left.has(copy.path);
    const folder = copyFolder(copy) || ctx.rootName;
    const keep = el("input", { type: "checkbox", className: "duplicate-copy-keep", checked: kept, ariaLabel: `Keep the copy in ${folder}` });
    keep.addEventListener("change", () => {
      state.setCopy(duplicate, copy.path, keep.checked ? "keep" : "trash");
      ctx.redraw.pane();
      // The copy's row is drawn again: the focus goes with it.
      pane.querySelector(`[data-path="${CSS.escape(copy.path)}"] .duplicate-copy-keep`)?.focus();
    });
    const row = el(
      "li",
      { className: `duplicate-copy ${kept ? "keep" : "trash"}` },
      keep,
      el(
        "span",
        { className: "duplicate-copy-where" },
        el("span", { className: "path duplicate-copy-folder", title: copy.path }, folder),
        el("span", { className: "pill duplicate-copy-tag" }, kept ? "Keep" : "Trash"),
        filed
          ? el(
              "span",
              { className: "pill duplicate-copy-tag", title: "This copy is also moved into a Suggested Folder; the other kept copies stay put." },
              "Filed",
            )
          : "",
      ),
      el("span", { className: "muted folder-file-date" }, formatDate(copy.modified)),
    );
    return ctx.selection.selectable(row, copy.path);
  }

  return { panel, trashNow: () => (trashedCopiesIn(state.duplicates) > 0 && !view.running ? trashCopiesNow : null) };
}
