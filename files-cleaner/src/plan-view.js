// What the Plan screen shows: the Operations grouped by Reason, and one group's files searched,
// sorted, filtered and paged. Operations are the Plan's, with `relativePath` and, for a
// duplicate, `keeper` as names from the Scan Root down; the Session screen lists its own
// Operations, which have `relativePath` too, with the same functions.

/** Which Reason an Operation is in. Rules are not planned yet; they would get their own group. */
function reasonOf(op) {
  return op.keeper ? "duplicates" : null;
}

const REASONS = ["duplicates"];

/** `[{ reason, operations }]`, one per Reason that has any Operations, in a fixed order. */
export function groupByReason(operations) {
  return REASONS.map((reason) => ({ reason, operations: operations.filter((op) => reasonOf(op) === reason) })).filter(
    (group) => group.operations.length > 0,
  );
}

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

/** How many copies a duplicate's Duplicate Group has, the Keeper included, given all of `operations`. */
export function copyCounts(operations) {
  const key = (op) => op.keeper.join("/");
  const counts = new Map();
  for (const op of operations) if (op.keeper) counts.set(key(op), (counts.get(key(op)) ?? 1) + 1);
  return (op) => counts.get(key(op));
}

/** "Photos/2025/" for the file's folder; the Scan Root's name, "Downloads/", for a top-level file. */
export function folderLabel(op, rootName) {
  return `${folderPath(op) || rootName}/`;
}
