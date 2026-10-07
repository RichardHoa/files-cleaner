// Searching and sorting the scan results table. Rows are `{ entry, path, extension, haystack }`,
// with `extension` and `haystack` already lowercased.

/**
 * "report .png .JPG" → { words: ["report"], extensions: Set {"png", "jpg"} }.
 * A word starting with a dot names an extension; a bare "." is ignored.
 */
export function parseSearch(text) {
  const words = [];
  const extensions = new Set();
  for (const word of text.toLowerCase().split(/\s+/).filter(Boolean)) {
    if (!word.startsWith(".")) words.push(word);
    else if (word.length > 1) extensions.add(word.slice(1));
  }
  return { words, extensions };
}

/** Every plain word is in the name or path, and the extension is one of those named (if any). */
export function matchesSearch(row, { words, extensions }) {
  return (extensions.size === 0 || extensions.has(row.extension)) && words.every((word) => row.haystack.includes(word));
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

const VALUES = {
  name: (row) => row.entry.name,
  extension: (row) => row.extension,
  size: (row) => row.entry.size,
  modified: (row) => row.entry.modified,
  created: (row) => row.entry.created,
  path: (row) => row.path,
};

/** Which way a column sorts on its first click: numbers and dates biggest first, text A→Z. */
export function defaultDirection(key) {
  return key === "size" || key === "modified" || key === "created" ? "desc" : "asc";
}

/** A sorted copy of `rows`. Unknown dates go last either way; ties keep their order. */
export function sortRows(rows, key, direction) {
  const value = VALUES[key];
  const sign = direction === "desc" ? -1 : 1;
  return [...rows].sort((a, b) => {
    const x = value(a);
    const y = value(b);
    if (x == null || y == null) return (x == null) - (y == null);
    return sign * (typeof x === "string" ? collator.compare(x, y) : x - y);
  });
}
