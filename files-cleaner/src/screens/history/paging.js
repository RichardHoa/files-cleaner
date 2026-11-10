// Which page links History shows: never all of them once there are many.

/** How many pages `total` items make, `perPage` to a page. */
export function pageCount(total, perPage) {
  return Math.ceil(total / perPage);
}

/**
 * The page links for page `current` of `count` (both from 1): the first and last pages, and the
 * current one with its neighbours. `null` marks a gap of two or more pages; a gap of one shows
 * that page instead, as "…" would take the same room.
 */
export function pageNumbers(current, count) {
  const wanted = [1, current - 1, current, current + 1, count].filter((n) => n >= 1 && n <= count);
  const pages = [...new Set(wanted)].sort((a, b) => a - b);
  const shown = [];
  for (const page of pages) {
    const previous = shown.at(-1);
    if (previous === page - 2) shown.push(page - 1);
    else if (previous != null && previous < page - 2) shown.push(null);
    shown.push(page);
  }
  return shown;
}
