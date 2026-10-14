// Which Plan Operations are ticked. `ticked` is a Set of Operation IDs; tree nodes carry
// `operationIds`, every Operation at or under them.

/** Totals for the ticked Operations: how many, how many of each kind, and bytes to be trashed. */
export function totals(operations, ticked) {
  const byKind = {};
  let count = 0;
  let trashBytes = 0;
  for (const op of operations) {
    if (!ticked.has(op.id)) continue;
    count += 1;
    byKind[op.kind] = (byKind[op.kind] ?? 0) + 1;
    if (op.kind === "trash") trashBytes += op.size;
  }
  return { count, byKind, trashBytes };
}

/** "all", "none" or "some": how many of the Operations under `node` are ticked. */
export function folderState(node, ticked) {
  const n = node.operationIds.filter((id) => ticked.has(id)).length;
  if (n === 0) return "none";
  return n === node.operationIds.length ? "all" : "some";
}

/** A new set with everything under `node` ticked, or all of it unticked if it was all ticked. */
export function toggleFolder(node, ticked) {
  const next = new Set(ticked);
  const untick = folderState(node, ticked) === "all";
  for (const id of node.operationIds) {
    if (untick) next.delete(id);
    else next.add(id);
  }
  return next;
}
