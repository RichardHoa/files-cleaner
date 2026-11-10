// Selecting rows of a list the way Finder does: a click selects one row, Cmd-click (Ctrl-click on
// Windows and Linux) adds or removes one, and Shift-click selects from the last row clicked to this one.

/**
 * The selection after a click on `item`, as a new Set. `order` is every item of the list in order,
 * `anchor` the item last clicked without Shift (or null), and `{ toggle, range }` whether Cmd or Shift was held.
 */
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
