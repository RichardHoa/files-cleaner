// The Review screen's Duplicates pane: the Duplicate Groups, each with the file's name once and its copies by
// folder under a "Keep" column, and "Keep only copies in". Ticking a copy's Keep box keeps it, any number of them
// per group; an unticked copy is marked Trash. The selection keeps or trashes any copies. They're trashed only
// with the rest of the Plan.
import { pathSeparator } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { drawMore } from "../../shared/plan/file-list.js";
import { formatDate, formatSize, plural, trashPlace } from "../../shared/format.js";
import { copyFolder, filedCopy, foldersHoldingCopies, trashedCopies, trashedCopiesIn } from "./review-choices.js";
import { copiesText, fileName } from "./pane-helpers.js";

/** The menu keeping the copies in one folder. */
const PREFER = "Keep only copies in";

/** The Duplicates pane of the Review screen with context `ctx`: `panel()` is what it lists. */
export function duplicatesPane(ctx) {
  const { state, view, pane } = ctx;

  /**
   * How many copies go to the System Trash, "Prefer copies in", then the Duplicate Groups, each with its
   * name, how many copies and the space going to the System Trash, then every copy.
   */
  function panel() {
    if (state.duplicates.length === 0) return [el("h2", {}, "Duplicates"), el("p", { className: "note" }, "No duplicates")];
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
      trashed ? el("p", { className: "muted" }, `${copiesText(trashed)} · ${formatSize(trashedSize)} → ${trashPlace(pathSeparator)}`) : "",
      el("div", { className: "duplicates-bulk" }, preferCopiesIn(), view.keepSkipped),
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
   * "Keep only copies in", a menu of the folders holding copies, starting on "Choose a folder…": keeps the copy
   * in the one picked and trashes the others, in the groups not changed by hand.
   */
  function preferCopiesIn() {
    const holding = foldersHoldingCopies(state.duplicates);
    const pick = el(
      "select",
      { ariaLabel: PREFER, title: "In every group with a copy there, keep that copy and trash the rest." },
      el("option", { value: "", disabled: true, selected: true }, "Choose a folder…"),
      ...holding.map((folder, i) => el("option", { value: i }, folder || ctx.rootName)),
    );
    pick.addEventListener("change", () => {
      const skipped = state.keepCopyIn(holding[Number(pick.value)]);
      view.keepSkipped = skipped ? el("span", { className: "muted", title: "You set these yourself." }, `${plural(skipped, "group")} unchanged`) : "";
      ctx.redraw.pane();
      // The menu is drawn again: the focus goes with it.
      pane.querySelector(`select[aria-label="${PREFER}"]`)?.focus();
    });
    return el("label", { className: "duplicates-prefer" }, el("span", {}, PREFER), pick);
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
    return ctx.selection.selectable(row, copy.path, { movable: false });
  }

  return { panel };
}
