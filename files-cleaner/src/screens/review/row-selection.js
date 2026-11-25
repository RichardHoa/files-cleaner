// The way Finder selects rows, which users expect on this screen.

/** `anchor` is the item last clicked without Shift. */
export function clickSelection(selection, order, anchor, item, { toggle = false, range = false } = {}) {
  const from = anchor === null ? -1 : order.indexOf(anchor);
  if (range && from !== -1) {
    const to = order.indexOf(item);
    const [start, end] = from < to ? [from, to] : [to, from];
    return new Set([...(toggle ? selection : []), ...order.slice(start, end + 1)]);
  }
  if (toggle) {
    const next = new Set(selection);
    if (next.has(item)) next.delete(item);
    else next.add(item);
    return next;
  }
  return new Set([item]);
}
