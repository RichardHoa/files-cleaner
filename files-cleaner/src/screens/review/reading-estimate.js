// Seconds per file is a run's whole time over the files it read, so loading the models and the last
// pass count too.

const FIRST_GUESS = 4;
/** In files. Before this many, this run's own pace isn't blended in. */
const SETTLED = 5;
const LONG_READ = 10 * 60;

/**
 * The stored average weighs as much as the first few files, so the estimate doesn't jump. It
 * counts down past the last file, to when the folders are shown.
 */
export function readingEstimate({ average, total, finished, elapsed }) {
  const stored = average ?? FIRST_GUESS;
  const perFile = finished < SETTLED ? stored : (stored * SETTLED + elapsed) / (SETTLED + finished);
  return { perFile, left: Math.max(0, perFile * total - elapsed) };
}

export function readingTime({ average, toRead }) {
  return (average ?? FIRST_GUESS) * toRead;
}

export function readingNeedsAsking({ average, toRead }) {
  return readingTime({ average, toRead }) > LONG_READ;
}

/** Each run counts for half, so the average follows this computer as it changes. */
export function updatedAverage(average, { seconds, files }) {
  if (files === 0) return average;
  const pace = seconds / files;
  return average == null ? pace : (average + pace) / 2;
}

export function describeEstimate({ perFile, left }, computer) {
  const seconds = Math.max(1, Math.round(perFile));
  const minutes = Math.round(left / 60);
  const remaining = minutes < 1 ? "less than a minute left" : `about ${minutes} min left`;
  return `About ${seconds} s per file on this ${computer} · ${remaining}`;
}
