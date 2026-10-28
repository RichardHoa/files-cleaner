// How long reading the files takes on this computer. The per-file time covers everything from
// starting to read until the Suggested Folders are shown (loading the models and the last pass
// included), so it is a run's whole time divided by the files it read. Files already read count as no
// time, so they are left out of the files.

/** The per-file time, in seconds, before any run on this computer. */
const FIRST_GUESS = 4;
/** How many files are read before this run's own pace is blended in. */
const SETTLED = 5;

/**
 * `{ perFile, left }` in seconds, from the stored `average` (or null), the `total` files to read,
 * how many are `finished`, and the seconds `elapsed` since reading started. After a few files this
 * run's pace is blended in, weighing the stored average as much as that many files. The time left
 * counts down to when the folders are shown, so it goes on through the last pass, once every file is read.
 */
export function readingEstimate({ average, total, finished, elapsed }) {
  const stored = average ?? FIRST_GUESS;
  const perFile = finished < SETTLED ? stored : (stored * SETTLED + elapsed) / (SETTLED + finished);
  return { perFile, left: Math.max(0, perFile * total - elapsed) };
}

/**
 * The stored average after a finished run that took `seconds` over `files` files; unchanged when it
 * read none. Each run counts for half, so the average follows this computer as it changes.
 */
export function updatedAverage(average, { seconds, files }) {
  if (files === 0) return average;
  const pace = seconds / files;
  return average == null ? pace : (average + pace) / 2;
}

/** "About 4 s per file on this Mac · about 3 min left". */
export function describeEstimate({ perFile, left }, computer) {
  const seconds = Math.max(1, Math.round(perFile));
  const minutes = Math.round(left / 60);
  const remaining = minutes < 1 ? "less than a minute left" : `about ${minutes} min left`;
  return `About ${seconds} s per file on this ${computer} · ${remaining}`;
}
