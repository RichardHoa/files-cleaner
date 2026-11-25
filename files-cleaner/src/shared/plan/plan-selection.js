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
