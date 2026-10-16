// What a Run, Undo or Redo did, from its `{ done, skipped, failed, unrecoverable }` summary.
import { el } from "./dom.js";
import { plural } from "./format.js";

/** "2 items restored · 1 skipped": `done` says what was done; failed and unrecoverable show only if any. */
export function describeSummary(summary, done) {
  const parts = [`${plural(summary.done.length, "item")} ${done}`, `${summary.skipped.length} skipped`];
  if (summary.failed.length > 0) parts.push(`${summary.failed.length} failed`);
  if (summary.unrecoverable.length > 0) parts.push(`${summary.unrecoverable.length} unrecoverable`);
  return parts.join(" · ");
}

/** What an Undo did: a line of counts, then the skipped and unrecoverable items with why. */
export function undoSections(summary) {
  return [
    el("p", { className: "lead" }, `Undo: ${describeSummary(summary, "restored")}`),
    ...outcomeSections(summary, { skipped: "Skipped by Undo", unrecoverable: "Unrecoverable" }),
  ];
}

/**
 * A heading and a list of items with their reasons for each of `summary`'s outcome lists named in
 * `titles` (`{ skipped: "Skipped", … }`), in that order; nothing for an empty list.
 */
export function outcomeSections(summary, titles) {
  return Object.entries(titles).flatMap(([field, title]) => outcomeSection(title, summary[field]));
}

function outcomeSection(title, outcomes) {
  if (outcomes.length === 0) return [];
  return [
    el("h2", {}, `${title} (${outcomes.length})`),
    el(
      "ul",
      { className: "outcomes" },
      ...outcomes.map((outcome) =>
        el("li", {}, el("span", { className: "path" }, outcome.source), outcome.reason ? el("div", { className: "reason" }, outcome.reason) : ""),
      ),
    ),
  ];
}
