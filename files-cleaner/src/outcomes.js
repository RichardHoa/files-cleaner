// What a Run, Undo or Redo did, from its `{ done, skipped, failed, unrecoverable }` summary.
import { el } from "./dom.js";
import { plural } from "./format.js";

/** "2 files restored · 1 skipped": `done` says what was done; failed and unrecoverable show only if any. */
export function describeSummary(summary, done) {
  const parts = [`${plural(summary.done.length, "file")} ${done}`, `${summary.skipped.length} skipped`];
  if (summary.failed.length > 0) parts.push(`${summary.failed.length} failed`);
  if (summary.unrecoverable.length > 0) parts.push(`${summary.unrecoverable.length} unrecoverable`);
  return parts.join(" · ");
}

/** What an Undo did: a line of counts, then the skipped and unrecoverable items with why. */
export function undoSections(summary) {
  return [
    el("p", { className: "lead" }, `Undo: ${describeSummary(summary, "restored")}`),
    ...outcomeSection("Skipped by Undo", summary.skipped),
    ...outcomeSection("Unrecoverable", summary.unrecoverable),
  ];
}

/** A heading and a list of `outcomes` with the note on each; nothing if there are none. */
function outcomeSection(title, outcomes) {
  if (outcomes.length === 0) return [];
  return [
    el("h2", {}, `${title} (${outcomes.length})`),
    el(
      "ul",
      { className: "outcomes" },
      ...outcomes.map((outcome) =>
        el("li", {}, el("span", { className: "path" }, outcome.source), outcome.note ? el("div", { className: "note" }, outcome.note) : ""),
      ),
    ),
  ];
}
