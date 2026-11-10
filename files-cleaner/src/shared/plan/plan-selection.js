// Ticking lists of Operations all at once. `ticked` is a Set of Operation IDs.

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
