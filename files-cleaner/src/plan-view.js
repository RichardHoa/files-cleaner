// What the Plan screen shows: the Operations grouped by Reason, and one group's files searched,
// sorted, filtered and paged. Operations are the Plan's, with `relativePath` and `destination` as
// names from the Scan Root down and `reason` either `{ type: "duplicate", keeper }` or
// `{ type: "rule", rule }`; the Session screen lists its own Operations, which have `relativePath`
// too, with the same functions.

import { splitPath } from "./format.js";

/**
 * `[{ reason, operations }]`, one per Reason that has any Operations: the duplicates first
 * (`reason: "duplicates"`), then one per Rule (`reason: "rule"`, with its name as `rule`) in the
 * order the Plan first names it.
 */
export function groupByReason(operations) {
  const duplicates = { reason: "duplicates", operations: [] };
  const rules = new Map();
  for (const op of operations) {
    if (op.reason.type === "duplicate") duplicates.operations.push(op);
    else {
      if (!rules.has(op.reason.rule)) rules.set(op.reason.rule, { reason: "rule", rule: op.reason.rule, operations: [] });
      rules.get(op.reason.rule).operations.push(op);
    }
  }
  return [duplicates, ...rules.values()].filter((group) => group.operations.length > 0);
}

/**
 * The folder the Operation puts its item in, for the user: `{ folder, insideRoot }`. Inside the
 * Scan Root `root` it is the names from there down (just the Holding Folder for Trash), `/`-separated;
 * elsewhere it is the full path. A folder to create is its own folder.
 */
export function destinationFolder(op, root, separator) {
  if (op.kind === "trash") return { folder: op.destination[0], insideRoot: true };
  const names = op.kind === "createFolder" ? op.destination : op.destination.slice(0, -1);
  const full = op.kind === "createFolder" ? op.destinationPath : splitPath(op.destinationPath, separator).parent;
  const insideRoot = op.destinationPath.startsWith(root + separator);
  return insideRoot ? { folder: names.join("/"), insideRoot } : { folder: full, insideRoot };
}

/** The name a Move gives its file, if that isn't the name it has now; otherwise null. */
export function newName(op) {
  if (op.kind !== "move") return null;
  const name = op.destination.at(-1);
  return name === op.relativePath.at(-1) ? null : name;
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

/**
 * How many copies a duplicate's Duplicate Group has, the Keeper included, given all of
 * `operations`; undefined for an Operation that isn't a duplicate's.
 */
export function copyCounts(operations) {
  const key = (op) => (op.reason.type === "duplicate" ? op.reason.keeper.join("/") : null);
  const counts = new Map();
  for (const op of operations) if (key(op)) counts.set(key(op), (counts.get(key(op)) ?? 1) + 1);
  return (op) => counts.get(key(op));
}

/** "Photos/2025/" for the file's folder; the Scan Root's name, "Downloads/", for a top-level file. */
export function folderLabel(op, rootName) {
  return `${folderPath(op) || rootName}/`;
}
