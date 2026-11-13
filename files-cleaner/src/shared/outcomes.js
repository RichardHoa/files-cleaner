// What a Run, Undo, Redo or retry did, from its `{ done, skipped, failed, unrecoverable }` summary.
import { el } from "./dom.js";
import { count, plural, splitPath, trashPlace } from "./format.js";

/** "2 files restored · 1 skipped": `done` says what was done; failed and unrecoverable show only if any. */
export function describeSummary(summary, done) {
  const parts = [`${plural(summary.done.length, "file")} ${done}`, `${summary.skipped.length} skipped`];
  if (summary.failed.length > 0) parts.push(`${summary.failed.length} failed`);
  if (summary.unrecoverable.length > 0) parts.push(`${summary.unrecoverable.length} unrecoverable`);
  return parts.join(" · ");
}

/**
 * Why a Run kept no Session, from its summary: "Nothing was moved. a.pdf: It no longer exists", from the first
 * Note. `done` is as for `describeSummary`, and `separator` is the OS's path separator.
 */
export function describeNothingDone(summary, done, separator) {
  const first = [...summary.skipped, ...summary.failed, ...summary.unrecoverable][0];
  // A System Trash case is worded as the warning words it.
  if (first?.noteKind) return `Nothing was ${done}. ${describeWarning(summary, { action: "run", done, separator })}`;
  const why = first ? ` ${splitPath(first.source, separator).name}: ${first.note}` : "";
  return `Nothing was ${done}.${why}`;
}

/** How the warning names each action, and since when a file it skipped as changed had changed. */
const ACTION_NAMES = {
  run: { name: "The Run", since: "since the scan" },
  retry: { name: "Trying again", since: "since the scan" },
  undo: { name: "Undo", since: "since it was trashed" },
  redo: { name: "Redo", since: "since it was put back" },
};

/**
 * The orange warning after a Run, Undo, Redo or retry (`action`: "run", "undo", "redo" or "retry") that left
 * files skipped, failed or unrecoverable: one or two plain sentences, the gravest first, telling the System
 * Trash cases apart by each outcome's `noteKind`. Null when everything worked. `done` says what was done, as
 * for `describeSummary`, and `separator` is the OS's path separator.
 */
export function describeWarning(summary, { action, done, separator }) {
  const notDone = [...summary.unrecoverable, ...summary.failed, ...summary.skipped];
  if (notDone.length === 0) return null;
  const trash = trashPlace(separator);
  const of = (kind) => notDone.filter((outcome) => outcome.noteKind === kind).length;
  // "it stayed where it was" or "they stayed where they were".
  const they = (n, one, many) => (n === 1 ? one : many);
  const { name, since } = ACTION_NAMES[action];
  const sentences = [];

  const gone = of("goneFromTrash");
  if (gone) {
    sentences.push(
      `${plural(gone, "file")} can't be brought back: ${they(gone, "it was", "they were")} emptied from the ${trash}, so ${they(gone, "it's", "they're")} gone for good.`,
    );
  }
  const refused = of("trashRefused");
  if (refused) {
    sentences.push(
      `${plural(refused, "file")} couldn't go to the ${trash} (on a drive with no ${trash} or a network folder, for example), so ${they(refused, "it stayed where it was", "they stayed where they were")}.`,
    );
  }
  const nameTaken = of("nameTaken");
  const changed = of("changed");
  if (nameTaken || changed) {
    const why = [
      nameTaken && "an item with the same name is in the way",
      changed && `${they(changed, "it", "they")} changed ${since}`,
    ].filter(Boolean);
    sentences.push(`${name} skipped ${plural(nameTaken + changed, "file")} because ${why.join(", or ")}.`);
  }
  const others = notDone.filter((outcome) => !outcome.noteKind);
  if (others.length === 1 && sentences.length === 0) {
    const [other] = others;
    sentences.push(`${splitPath(other.source, separator).name} wasn't ${done}${other.note ? `: ${other.note}` : ""}.`);
  } else if (others.length) {
    const files = sentences.length ? plural(others.length, "other file") : plural(others.length, "file");
    sentences.push(`${files} ${they(others.length, "wasn't", "weren't")} ${done}. Each says why.`);
  }
  return sentences.slice(0, 2).join(" ");
}

/**
 * The orange warning block with `text`, centered above what it is about, and "Show which files" calling
 * `onShow` if given. It has no close button: it goes when the screen does.
 */
export function warningBlock(text, onShow = null) {
  const node = el("div", { className: "warning-block", role: "alert" }, el("p", {}, text));
  if (onShow) {
    const link = el("button", { type: "button", className: "link" }, "Show which files");
    link.addEventListener("click", onShow);
    node.append(link);
  }
  return node;
}

/** "Moving 120 of 3,000 files…" from a Run's `{ finished, total }` progress, then the item it is on. */
export function describeRunProgress({ finished, total, current }) {
  return `Moving ${count(finished)} of ${plural(total, "item")}…\n${current}`;
}

/**
 * What an Undo did: the warning if it left any files, a line of counts, then the skipped and unrecoverable
 * items with why. `separator` is the OS's path separator.
 */
export function undoSections(summary, separator) {
  const sections = [...outcomeSection("Unrecoverable", summary.unrecoverable), ...outcomeSection("Skipped by Undo", summary.skipped)];
  const warning = describeWarning(summary, { action: "undo", done: "put back", separator });
  return [
    warning ? warningBlock(warning, () => sections[0]?.scrollIntoView({ behavior: "smooth", block: "start" })) : "",
    el("p", { className: "lead" }, `Undo: ${describeSummary(summary, "restored")}`),
    ...sections,
  ];
}

/** What a Run left: its unrecoverable, failed and skipped items with why, under a heading each. */
export function notDoneSections(summary) {
  return [
    ...outcomeSection("Unrecoverable", summary.unrecoverable),
    ...outcomeSection("Failed", summary.failed),
    ...outcomeSection("Skipped", summary.skipped),
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
