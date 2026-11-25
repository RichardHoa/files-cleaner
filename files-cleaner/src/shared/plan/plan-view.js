// PAGE_SIZE also pages the review screen.

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
