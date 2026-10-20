// Which Plan Operations are ticked. `ticked` is a Set of Operation IDs.

/** The Operations ticked when the Plan is first shown: those it ticks, never one with an error. */
export function initiallyTicked(operations) {
  return new Set(operations.filter((op) => op.ticked && !op.error).map((op) => op.id));
}

/** The IDs of the `operations` that can be ticked: all but those with an error. */
export function tickableIds(operations) {
  return operations.filter((op) => !op.error).map((op) => op.id);
}

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

/** "all", "none" or "some": how many of the Operations `ids` are ticked. */
export function tickState(ids, ticked) {
  const n = ids.filter((id) => ticked.has(id)).length;
  if (n === 0) return "none";
  return n === ids.length ? "all" : "some";
}

/** A new set with all of `ids` ticked, or all of them unticked if they all were. */
export function toggleAll(ids, ticked) {
  const next = new Set(ticked);
  const untick = tickState(ids, ticked) === "all";
  for (const id of ids) {
    if (untick) next.delete(id);
    else next.add(id);
  }
  return next;
}
