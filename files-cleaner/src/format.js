const UNITS = ["bytes", "KB", "MB", "GB", "TB"];

/** 1536 → "1.5 KB", like Finder (powers of 1000). */
export function formatSize(bytes) {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  if (unit === 0) return `${bytes} ${bytes === 1 ? "byte" : "bytes"}`;
  return `${value.toFixed(value < 10 ? 1 : 0)} ${UNITS[unit]}`;
}

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

/** Milliseconds since the epoch → a local date and time; "—" if unknown. */
export function formatDate(millis) {
  return millis == null ? "—" : dateFormat.format(new Date(millis));
}

const numberFormat = new Intl.NumberFormat();

/** 108000 → "108,000". */
export function count(n) {
  return numberFormat.format(n);
}

/** `plural(3, "Rule")` → "3 Rules"; `plural(1, "Rule")` → "1 Rule". */
export function plural(n, word) {
  return `${count(n)} ${n === 1 ? word : `${word}s`}`;
}

/**
 * "a/b/c/d.txt" → "…/c/d.txt": the longest end of `path` that `fits`, led by "…". Whole
 * folders (ended by the one-character `separator`) are dropped from the start first; only a
 * file name too long on its own is cut mid-word.
 * If `fits` accepts a text, it must also accept every shorter end of it.
 */
export function fitTail(path, separator, fits) {
  if (fits(path)) return path;
  const tail = (start) => `…${path.slice(start)}`;
  const breaks = [];
  for (let i = path.indexOf(separator, 1); i !== -1; i = path.indexOf(separator, i + 1)) breaks.push(i);
  const lastBreak = breaks.at(-1) ?? -1;
  const atBreak = firstFitting(breaks, (start) => fits(tail(start)));
  if (atBreak !== undefined) return tail(atBreak);
  const starts = Array.from({ length: path.length - lastBreak - 2 }, (_, i) => lastBreak + 2 + i);
  return tail(firstFitting(starts, (start) => fits(tail(start))) ?? path.length - 1);
}

/** The first of `sorted` that passes `ok`, given every later one passes once one does. */
function firstFitting(sorted, ok) {
  let low = 0;
  let high = sorted.length;
  while (low < high) {
    const mid = (low + high) >> 1;
    if (ok(sorted[mid])) high = mid;
    else low = mid + 1;
  }
  return sorted[low];
}
