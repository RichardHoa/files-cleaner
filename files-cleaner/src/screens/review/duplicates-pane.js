// The Review screen's Duplicates pane: the Duplicate Groups, each copy kept or trashed, "Keep the copy in…",
// and trashing the copies as a Session of their own, without waiting for the reading.
import { pathSeparator } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { drawMore } from "../../shared/plan/file-list.js";
import { count, formatDate, formatSize, plural, trashPlace } from "../../shared/format.js";
import { copyFolder, filedCopy, foldersHoldingCopies, planDuplicates, trashedCopies, trashedCopiesIn } from "./review-choices.js";
import { copiesText, fileName } from "./pane-helpers.js";

/** The Duplicates pane of the Review screen with context `ctx`: `panel()` is what it lists. */
export function duplicatesPane(ctx) {
  const { state, view, pane } = ctx;

  /** Trashes the copies marked Trash as a Session of their own, leaving the rest of the Plan to review. */
  async function trashDuplicates(button) {
    const summary = await ctx.sessions.runPart(button, { duplicates: planDuplicates(state.duplicates) }, "Trashing the copies…", "trashed");
    if (summary) state.ranDuplicates(summary);
    ctx.redraw.section("duplicates");
  }

  /**
   * How many copies go to the System Trash, then the Duplicate Groups, each with its name, how many
   * copies and the space going to the System Trash, then every copy, with a button trashing the copies
   * marked Trash and a menu keeping the copies in a folder; once that's done, what it did.
   */
  function panel() {
    if (state.duplicates.length === 0) return [el("h2", {}, "Duplicates"), el("p", { className: "note" }, "No duplicates")];
    if (state.duplicatesDone) {
      return [el("h2", {}, "Duplicates"), ...ctx.sessions.outcome(state.duplicatesDone, "trashed")];
    }
    const trashed = trashedCopiesIn(state.duplicates);
    const trashedSize = state.duplicates.reduce((sum, duplicate) => sum + duplicate.group.size * trashedCopies(duplicate), 0);
    const shown = Math.min(state.duplicates.length, view.duplicatesShown);
    const more = el("div", { className: "plan-more" });
    drawMore(more, state.duplicates.length, shown, (limit) => {
      view.duplicatesShown = limit;
      ctx.redraw.pane();
    });
    const trash = el("button", { type: "button", disabled: view.running || trashed === 0 }, trashed ? `Trash just these ${copiesText(trashed)}` : "Nothing to trash");
    trash.addEventListener("click", () => trashDuplicates(trash));
    return [
      el("div", { className: "duplicates-head" }, el("h2", {}, "Duplicates"), trash),
      trashed ? el("p", { className: "muted" }, `${copiesText(trashed)} · ${formatSize(trashedSize)} → ${trashPlace(pathSeparator)}`) : "",
      el("div", { className: "duplicates-bulk" }, keepCopyInMenu(), view.keepSkipped),
      el("ul", { className: "duplicate-groups", ariaLabel: "Duplicate Groups" }, ...state.duplicates.slice(0, shown).map(duplicateGroup)),
      more,
    ];
  }

  /** A menu of the folders holding copies, keeping the copy in the one picked and trashing the others, in the groups not changed by hand. */
  function keepCopyInMenu() {
    const label = "Keep the copy in…";
    const holding = foldersHoldingCopies(state.duplicates);
    const pick = el(
      "select",
      { ariaLabel: label },
      el("option", { value: "", disabled: true, selected: true }, label),
      ...holding.map((folder, i) => el("option", { value: i }, folder || ctx.rootName)),
    );
    pick.value = "";
    pick.addEventListener("change", () => {
      const skipped = state.keepCopyIn(holding[Number(pick.value)]);
      view.keepSkipped = skipped
        ? el(
            "span",
            { className: "muted", title: "You set these yourself, or they have already run." },
            `${plural(skipped, "group")} unchanged`,
          )
        : "";
      ctx.redraw.pane();
      // The menu is drawn again: the focus goes with it.
      pane.querySelector(`select[aria-label="${label}"]`)?.focus();
    });
    return pick;
  }

  /** A Duplicate Group: its name, how many copies, the space going to the System Trash, a warning if no copy is kept, then each copy. */
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
        el(
          "span",
          { className: "muted" },
          ` · ${count(group.copies.length)} copies · ${formatSize(group.size * trashedCopies(duplicate))} to ${trashPlace(pathSeparator)}`,
          filed ? "" : el("span", { className: "warning" }, " · No copy kept"),
        ),
      ),
      el("ul", { className: "duplicate-copies" }, ...group.copies.map((copy) => duplicateCopy(duplicate, copy))),
    );
  }

  /**
   * One copy of the Duplicate Group `duplicate`: Keep or Trash, then its name, with its full path on hover, its
   * folder from the Scan Root and its modified date. Neither can be picked once a copy of the group was run.
   */
  function duplicateCopy(duplicate, copy) {
    const fate = duplicate.copies.get(copy.path);
    const own = fileName(copy.path);
    const segments = [
      ["keep", "Keep"],
      ["trash", "Trash"],
    ].map(([value, label]) => {
      const segment = el("button", { type: "button", role: "radio", className: "segment", ariaChecked: String(value === fate), disabled: state.locked(duplicate) }, label);
      segment.addEventListener("click", () => {
        if (value === fate) return;
        state.setCopy(duplicate, copy.path, value);
        ctx.redraw.pane();
        // The copy's row is drawn again: the focus goes with it.
        pane.querySelector(`[data-copy="${CSS.escape(copy.path)}"] .segment[aria-checked="true"]`)?.focus();
      });
      return segment;
    });
    const row = el(
      "li",
      { className: `duplicate-copy ${fate}` },
      el("div", { className: "segmented", role: "radiogroup", ariaLabel: `What to do with ${own}` }, ...segments),
      el("span", { className: "duplicate-copy-name", title: copy.path }, own),
      el("span", { className: "path duplicate-copy-folder" }, copyFolder(copy) || ctx.rootName),
      el("span", { className: "muted folder-file-date" }, formatDate(copy.modified)),
    );
    row.dataset.copy = copy.path;
    return ctx.selection.selectable(row, copy.path, { movable: false });
  }

  return { panel };
}
