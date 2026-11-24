// The Review screen's Duplicates pane: the Duplicate Groups, each with the file's name once and its copies by
// folder under a "Keep" column. Ticking a copy's Keep box keeps it, any number of them per group; an unticked
// copy is marked Trash. The selection keeps or trashes any copies, from its right-click menu or ⌘⌫. The copies marked Trash go with the rest of the Plan, or on their own, at once, from the button
// above the groups: the Suggested Folders stay as they are, for the header's button.
import { buildReviewPlan, pathSeparator, runPlan } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { drawMore } from "../../shared/plan/file-list.js";
import { formatDate, formatSize, trashPlace } from "../../shared/format.js";
import { icon } from "../../shared/icons.js";
import { withShortcut } from "../../shared/menus/menu-items.js";
import { describeNothingDone, describeRunProgress, describeWarning } from "../../shared/outcomes.js";
import { busy, showStatus, showWarning } from "../../shared/status.js";
import { copyFolder, filedCopy, planDuplicates, trashedCopies, trashedCopiesIn } from "./review-choices.js";
import { copiesText, fileName } from "./pane-helpers.js";

/**
 * The Duplicates pane of the Review screen with context `ctx`: `panel()` is what it lists, and `trashNow()` trashes
 * the copies marked Trash now, or is null when there are none or a Session is running.
 */
export function duplicatesPane(ctx) {
  const { state, view, pane } = ctx;
  const trashNow = el("button", { type: "button", className: "small" }, icon("trash"), el("span"));
  // What trashing the copies now did, kept across redraws.
  const status = el("p", { className: "status duplicates-trash-status" });
  trashNow.addEventListener("click", trashCopiesNow);

  /**
   * The button trashing the copies marked Trash now, with how much space that frees, then the Duplicate
   * Groups, each with its name, how many copies and the space going to the System Trash, then every copy.
   */
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
      trashNowRow(trashed, trashedSize),
      status,
      el(
        "div",
        { className: "duplicate-columns muted", ariaHidden: "true" },
        el("span", {}, "Keep"),
        el("span", {}, "Folder"),
        el("span", {}, "Modified"),
      ),
      el("ul", { className: "duplicate-groups", ariaLabel: "Duplicate Groups" }, ...state.duplicates.slice(0, shown).map(duplicateGroup)),
      more,
    ];
  }

  /**
   * "Trash N copies now", trashing every unticked copy at once, on its own, beside what it frees, with a
   * tooltip saying the Suggested Folders aren't touched.
   */
  function trashNowRow(trashed, trashedSize) {
    const trash = trashPlace(pathSeparator);
    trashNow.lastChild.textContent = trashed ? `Trash ${copiesText(trashed)} now` : "No copies to trash";
    trashNow.disabled = trashed === 0 || view.running;
    trashNow.title = withShortcut(`Moves every unticked copy to the ${trash} now. Suggested Folders aren't touched. Undo it from History.`, "trashNow");
    return el(
      "div",
      { className: "duplicates-trash-now" },
      trashNow,
      trashed ? el("span", { className: "muted" }, `Unticked copies only · ${formatSize(trashedSize)} → ${trash}`) : "",
    );
  }

  /**
   * Trashes the copies marked Trash as a Session of their own, and stays on the Review screen, the copies
   * trashed gone from it; any copy that couldn't go stays marked Trash, for the rest of the Plan, and is said why.
   */
  async function trashCopiesNow() {
    if (view.running) return;
    view.running = true;
    ctx.redraw.header();
    const summary = await busy(status, [trashNow], "Trashing copies…", async () => {
      const plan = await buildReviewPlan(planDuplicates(state.duplicates), []);
      const approved = plan.operations.filter((op) => op.reason.type === "duplicate" && (op.ticked || Boolean(op.error))).map((op) => op.id);
      return runPlan(approved, (progress) => showStatus(status, describeRunProgress(progress)));
    });
    view.running = false;
    if (summary) {
      state.copiesTrashed(summary.done.map((outcome) => outcome.source));
      const warning = describeWarning(summary, { action: "run", done: "trashed", separator: pathSeparator });
      // A Run that did nothing keeps no Session.
      if (!summary.sessionId) showWarning(status, describeNothingDone(summary, "trashed", pathSeparator));
      else if (warning) showWarning(status, warning);
      else showStatus(status, `${copiesText(summary.done.length)} trashed · undo from History`);
    }
    ctx.redraw.pane();
  }

  /** A Duplicate Group: its name, with a warning if no copy is kept, then each copy. */
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

  /**
   * One copy of the Duplicate Group `duplicate`: its Keep box, then its folder from the Scan Root, with its full
   * path on hover, and its modified date. A trashed copy is dimmed and struck through.
   */
  function duplicateCopy(duplicate, copy) {
    const kept = duplicate.copies.get(copy.path) === "keep";
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
      el("span", { className: "path duplicate-copy-folder", title: copy.path }, folder),
      el("span", { className: "muted folder-file-date" }, formatDate(copy.modified)),
    );
    return ctx.selection.selectable(row, copy.path);
  }

  return { panel, trashNow: () => (trashedCopiesIn(state.duplicates) > 0 && !view.running ? trashCopiesNow : null) };
}
