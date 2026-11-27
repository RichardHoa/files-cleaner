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

// A guess this early swings wildly, and finer rounding would jump about with each report.

/** In ms. */
const WARM_UP = 3000;
/** In Operations. */
const RATE_WINDOW = 50;

/** `samples` are `{ at, finished }` with `at` in ms, oldest first. "" while there's no fair guess. */
export function timeLeft(startedAt, samples, total) {
  const last = samples.at(-1);
  if (!last || last.at - startedAt < WARM_UP) return "";
  const from = samples.findLast((sample) => sample.finished <= last.finished - RATE_WINDOW) ?? samples[0];
  if (last.finished <= from.finished || last.at <= from.at) return "";
  const minutes = Math.round(((total - last.finished) * (last.at - from.at)) / (last.finished - from.finished) / 60_000);
  if (minutes < 1) return "less than a minute left";
  if (minutes < 60) return `about ${minutes} min left`;
  return `about ${Math.round(minutes / 60)} hr left`;
}

/** Keeps just enough for `timeLeft`, so a long Run doesn't pile them up. */
export function addSample(samples, sample) {
  const kept = [...samples, sample];
  while (kept.length > 2 && kept[1].finished <= sample.finished - RATE_WINDOW) kept.shift();
  return kept;
}

/** A disk root has no parent, so it is its own name. */
export function splitPath(path, separator) {
  const trimmed = path.length > 1 && path.endsWith(separator) ? path.slice(0, -1) : path;
  const at = trimmed.lastIndexOf(separator);
  if (at === -1 || at === trimmed.length - 1) return { name: path, parent: "" };
  return { name: trimmed.slice(at + 1), parent: at === 0 ? separator : trimmed.slice(0, at) };
}

export const trashPlace = (separator) => (separator === "\\" ? "Recycle Bin" : "Trash");

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

export const fileName = (op) => op.relativePath.at(-1);

const folderPath = (op) => op.relativePath.slice(0, -1).join("/");

const bySize = (a, b) => b.size - a.size;
const byName = (a, b) => collator.compare(fileName(a), fileName(b));

const SORTS = {
  size: (a, b) => bySize(a, b) || byName(a, b),
  name: (a, b) => byName(a, b) || collator.compare(folderPath(a), folderPath(b)),
  folder: (a, b) => collator.compare(folderPath(a), folderPath(b)) || bySize(a, b) || byName(a, b),
};

export function visibleFiles(operations, { query, sort, show = "all" }, ticked = new Set()) {
  const needle = query.trim().toLowerCase();
  return operations
    .filter((op) => show === "all" || (show === "ticked") === ticked.has(op.id))
    .filter((op) => op.relativePath.join("/").toLowerCase().includes(needle))
    .sort(SORTS[sort]);
}

/** Also pages the review screen. */
export const PAGE_SIZE = 50;

export function paging(total, limit) {
  const shown = Math.min(total, limit);
  return { shown, more: Math.min(PAGE_SIZE, total - shown) };
}

export function selectedFile(shown, id) {
  return shown.find((op) => op.id === id) ?? shown[0] ?? null;
}

export function folderLabel(op, rootName) {
  return `${folderPath(op) || rootName}/`;
}

export function tickState(ids, ticked) {
  const n = ids.filter((id) => ticked.has(id)).length;
  if (n === 0) return "none";
  return n === ids.length ? "all" : "some";
}

export function toggleAll(ids, ticked) {
  const next = new Set(ticked);
  const untick = tickState(ids, ticked) === "all";
  for (const id of ids) {
    if (untick) next.delete(id);
    else next.add(id);
  }
  return next;
}
