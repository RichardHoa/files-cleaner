import { pathSeparator } from "./api.js";
import { el, showStatus } from "./dom.js";
import { addSample, count, plural, splitPath, timeLeft, trashPlace } from "./format.js";

/** Failed and unrecoverable show only if any. */
export function describeSummary(summary, done) {
  const parts = [`${plural(summary.done.length, "file")} ${done}`, `${summary.skipped.length} skipped`];
  if (summary.failed.length > 0) parts.push(`${summary.failed.length} failed`);
  if (summary.unrecoverable.length > 0) parts.push(`${summary.unrecoverable.length} unrecoverable`);
  return parts.join(" · ");
}

/** With no Session kept, this is the only place the user learns why. */
export function describeNothingDone(summary, done, separator) {
  const first = [...summary.skipped, ...summary.failed, ...summary.unrecoverable][0];
  // Worded as the warning words it.
  if (first?.noteKind) return `Nothing was ${done}. ${describeWarning(summary, { action: "run", done, separator })}`;
  const why = first ? ` ${splitPath(first.source, separator).name}: ${first.note}` : "";
  return `Nothing was ${done}.${why}`;
}

const ACTION_NAMES = {
  run: { name: "The Run", since: "since the scan" },
  retry: { name: "Trying again", since: "since the scan" },
  undo: { name: "Undo", since: "since it was trashed" },
  redo: { name: "Redo", since: "since it was put back" },
};

/** The gravest first. `noteKind` tells the System Trash cases apart. */
export function describeWarning(summary, { action, done, separator }) {
  const notDone = [...summary.unrecoverable, ...summary.failed, ...summary.skipped];
  if (notDone.length === 0) return null;
  const trash = trashPlace(separator);
  const of = (kind) => notDone.filter((outcome) => outcome.noteKind === kind).length;
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

/** No close button: it goes when the screen does. */
export function warningBlock(text, onShow = null) {
  const node = el("div", { className: "warning-block", role: "alert" }, el("p", {}, text));
  if (onShow) {
    const link = el("button", { type: "button", className: "link" }, "Show which files");
    link.addEventListener("click", onShow);
    node.append(link);
  }
  return node;
}

/** `of` names the total, as in "2,040 copies". */
export function describeRunProgress({ finished, total, phase }, { verb, of }, left = "") {
  const done = phase === "puttingBack" ? `Putting back ${count(finished)} of ${count(total)}` : `${verb} ${count(finished)} of ${of(total)}`;
  return `${done}${left ? ` · ${left}` : ""}`;
}

/**
 * Two lines that keep their size whatever the path, so a long one can't push the screen about.
 * The path is cut in its middle: the file name always shows.
 */
export function showRunProgress(node, words) {
  let startedAt = Date.now();
  let samples = [];
  let phase = "running";
  return (progress) => {
    // The put-back has its own pace.
    if (progress.phase !== phase) {
      phase = progress.phase;
      startedAt = Date.now();
      samples = [];
    }
    samples = addSample(samples, { at: Date.now(), finished: progress.finished });
    const { name, parent } = splitPath(progress.current, pathSeparator);
    const folder = parent && !parent.endsWith(pathSeparator) ? `${parent}${pathSeparator}` : parent;
    showStatus(node, "");
    node.classList.add("progress-line");
    node.append(
      el("span", { className: "progress-line-count" }, describeRunProgress(progress, words, timeLeft(startedAt, samples, progress.total))),
      el(
        "span",
        { className: "path progress-line-path", title: progress.current },
        el("span", { className: "progress-line-folder" }, folder),
        el("span", { className: "progress-line-name" }, name),
      ),
    );
  };
}

export function undoSections(summary, separator) {
  const sections = [...outcomeSection("Unrecoverable", summary.unrecoverable), ...outcomeSection("Skipped by Undo", summary.skipped)];
  const warning = describeWarning(summary, { action: "undo", done: "put back", separator });
  return [
    warning ? warningBlock(warning, () => sections[0]?.scrollIntoView({ behavior: "smooth", block: "start" })) : "",
    el("p", { className: "lead" }, `Undo: ${describeSummary(summary, "restored")}`),
    ...sections,
  ];
}

export function notDoneSections(summary) {
  return [
    ...outcomeSection("Unrecoverable", summary.unrecoverable),
    ...outcomeSection("Failed", summary.failed),
    ...outcomeSection("Skipped", summary.skipped),
  ];
}

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
