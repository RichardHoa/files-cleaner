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

/** `plural(3, "Rule")` → "3 Rules"; `plural(1, "Rule")` → "1 Rule". */
export function plural(count, word) {
  return `${count} ${count === 1 ? word : `${word}s`}`;
}
