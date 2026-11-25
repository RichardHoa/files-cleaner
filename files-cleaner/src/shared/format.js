const UNITS = ["bytes", "KB", "MB", "GB", "TB"];

/** Powers of 1000, like macOS and most Linux file managers. */
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

export function formatDate(millis) {
  return millis == null ? "—" : dateFormat.format(new Date(millis));
}

const numberFormat = new Intl.NumberFormat();

export function count(n) {
  return numberFormat.format(n);
}

export function plural(n, word) {
  return `${count(n)} ${n === 1 ? word : `${word}s`}`;
}

/** Binary search, so if `fits` accepts a text it must accept every shorter end of it. */
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

/** `sorted` must fail up to some point and pass from there on. */
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

const RELATIVE_UNITS = [
  ["year", 365 * 24 * 60 * 60_000],
  ["month", 30 * 24 * 60 * 60_000],
  ["day", 24 * 60 * 60_000],
  ["hour", 60 * 60_000],
  ["minute", 60_000],
];

export function relativeTime(millis, now = Date.now()) {
  const elapsed = now - millis;
  for (const [unit, length] of RELATIVE_UNITS) {
    const n = Math.floor(elapsed / length);
    if (n >= 1) return `${plural(n, unit)} ago`;
  }
  return "just now";
}

/** A disk root has no parent, so it is its own name. */
export function splitPath(path, separator) {
  const trimmed = path.length > 1 && path.endsWith(separator) ? path.slice(0, -1) : path;
  const at = trimmed.lastIndexOf(separator);
  if (at === -1 || at === trimmed.length - 1) return { name: path, parent: "" };
  return { name: trimmed.slice(at + 1), parent: at === 0 ? separator : trimmed.slice(0, at) };
}

export const trashPlace = (separator) => (separator === "\\" ? "Recycle Bin" : "Trash");
