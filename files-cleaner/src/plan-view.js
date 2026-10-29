// How the Session screen lists a group of its Operations: searched, sorted, filtered and paged.
// Each Operation has `relativePath`, the names from the Scan Root down to its item.

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

/** The file's own name. */
export const fileName = (op) => op.relativePath.at(-1);

/** The folders from the Scan Root down to the file, `/`-separated; "" at the top. */
const folderPath = (op) => op.relativePath.slice(0, -1).join("/");

const bySize = (a, b) => b.size - a.size;
const byName = (a, b) => collator.compare(fileName(a), fileName(b));

const SORTS = {
  size: (a, b) => bySize(a, b) || byName(a, b),
  name: (a, b) => byName(a, b) || collator.compare(folderPath(a), folderPath(b)),
  folder: (a, b) => collator.compare(folderPath(a), folderPath(b)) || bySize(a, b) || byName(a, b),
};

/**
 * The files of `operations` to list: those whose path from the Scan Root contains `query`
 * (ignoring case) and that `show` picks ("all", the default, "ticked" or "unticked"), sorted by
 * `sort` ("size", biggest first; "name"; or "folder"). Also lists a Session's Operations.
 */
export function visibleFiles(operations, { query, sort, show = "all" }, ticked = new Set()) {
  const needle = query.trim().toLowerCase();
  return operations
    .filter((op) => show === "all" || (show === "ticked") === ticked.has(op.id))
    .filter((op) => op.relativePath.join("/").toLowerCase().includes(needle))
    .sort(SORTS[sort]);
}

/** How many files a page holds; "Show more" adds another page. */
export const PAGE_SIZE = 50;

/** Of `total` files with up to `limit` shown: how many are shown, and how many "Show more" would add. */
export function paging(total, limit) {
  const shown = Math.min(total, limit);
  return { shown, more: Math.min(PAGE_SIZE, total - shown) };
}

/** The shown file with ID `id`; if it isn't shown, the first that is; null if none are. */
export function selectedFile(shown, id) {
  return shown.find((op) => op.id === id) ?? shown[0] ?? null;
}

/** "Photos/2025/" for the file's folder; the Scan Root's name, "Downloads/", for a top-level file. */
export function folderLabel(op, rootName) {
  return `${folderPath(op) || rootName}/`;
}
