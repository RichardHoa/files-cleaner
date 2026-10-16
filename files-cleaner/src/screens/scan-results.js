// Scan results: every scanned file and Package, plus what was left alone.
// Scans can list 100,000+ items, so the table only builds the rows in view.
import { buildPlan, describeError, pathSeparator, revealInFinder } from "../api.js";
import { el } from "../dom.js";
import { count, fitTail, formatDate, formatSize, plural } from "../format.js";
import { icon } from "../icons.js";
import { defaultDirection, matchesSearch, parseSearch, sortRows } from "../scan-query.js";
import { busy } from "../status.js";

// `track` is the column's default width; `min` is the least an `fr` track shrinks to.
const COLUMNS = [
  { key: "name", label: "Name", track: "minmax(160px, 2fr)", min: 160 },
  { key: "extension", label: "Extension", track: "90px", min: 90 },
  { key: "size", label: "Size", track: "90px", min: 90, className: "number" },
  { key: "modified", label: "Modified", track: "170px", min: 170 },
  { key: "created", label: "Created", track: "170px", min: 170 },
  { key: "path", label: "Path", track: "minmax(120px, 1fr)", min: 120 },
];
const PATH_COLUMN = COLUMNS.findIndex((column) => column.key === "path");
const ROW_HEIGHT = 44; // px; must match `.scan-row` in styles/scan-results.css
const LINE_HEIGHT = 18; // px; must match `.scan-cell` in styles.css
const CELL_PADDING = 16; // px of left plus right padding; must match `.scan-cell` in styles.css
const MIN_COLUMN_WIDTH = 60; // px a dragged column can shrink to
const WIDTHS_KEY = "scan-results.column-widths";
const OVERSCAN = 10; // rows built above and below the visible ones
const SEARCH_DELAY = 120; // ms of no typing before the table refilters

const MIN_SIZES = [
  ["Any size", 0],
  ["Over 1 MB", 1e6],
  ["Over 10 MB", 1e7],
  ["Over 100 MB", 1e8],
  ["Over 1 GB", 1e9],
];

export function render(root, nav, result) {
  const back = el("button", { type: "button" }, "Back");
  back.addEventListener("click", () => nav.home());
  const plan = el("button", { type: "button", className: "primary" }, "Build Plan");
  const planStatus = el("p", { className: "status" });
  plan.addEventListener("click", async () => {
    const built = await busy(planStatus, [plan], "Looking for duplicates…", buildPlan);
    if (built) nav.plan(result, built);
  });

  const totalSize = result.entries.reduce((sum, entry) => sum + entry.size, 0);
  const summary = [
    plural(result.entries.length, "item"),
    formatSize(totalSize),
    `${plural(result.skipped, "Protected Path")} skipped`,
    `${plural(result.noExtension, "file")} with no extension left out`,
  ];
  if (result.unreadable.length > 0) summary.push(`${count(result.unreadable.length)} couldn't be read`);

  const rows = result.entries.map((entry) => {
    const path = entry.relativePath.join(pathSeparator);
    return { entry, path, extension: entry.extension.toLowerCase(), haystack: `${entry.name}\n${path}`.toLowerCase() };
  });

  const search = el("input", { type: "search", placeholder: "Search name or path, or .ext", className: "search" });
  const extension = el("select", {}, el("option", { value: "" }, `All extensions`), ...extensionOptions(rows));
  const minSize = el("select", {}, ...MIN_SIZES.map(([label, bytes]) => el("option", { value: bytes }, label)));
  const shown = el("p", { className: "shown" });

  // Largest first: the biggest items are usually what's worth cleaning.
  let sort = { key: "size", direction: "desc" };
  let sorted = sortRows(rows, sort.key, sort.direction);
  const list = virtualList(sort, (key) => {
    const direction = key !== sort.key ? defaultDirection(key) : sort.direction === "asc" ? "desc" : "asc";
    sort = { key, direction };
    sorted = sortRows(rows, key, direction);
    list.setSort(sort);
    refilter();
  });

  const screen = el(
    "div",
    { className: "scan-screen" },
    el("div", { className: "actions" }, back, plan),
    planStatus,
    el("h1", {}, "Scan results"),
    el("p", { className: "path root-path" }, icon("folder"), result.root),
    el("p", { className: "lead" }, summary.join(" · ")),
  );
  if (result.unreadable.length > 0) {
    screen.append(
      el(
        "details",
        {},
        el("summary", {}, `Couldn't be read (${count(result.unreadable.length)})`),
        el("ul", { className: "path-list" }, ...result.unreadable.map((path) => el("li", {}, path))),
      ),
    );
  }
  screen.append(el("div", { className: "filters" }, search, extension, minSize, shown), list.node);
  root.append(screen);

  const refilter = () => {
    const query = parseSearch(search.value);
    const wantExtension = extension.value;
    const wantSize = Number(minSize.value);
    const matches = sorted.filter(
      (row) =>
        (!wantExtension || row.extension === wantExtension) && row.entry.size >= wantSize && matchesSearch(row, query),
    );
    const size = matches.reduce((sum, row) => sum + row.entry.size, 0);
    shown.textContent =
      matches.length === rows.length
        ? `Showing all ${plural(rows.length, "item")}`
        : `Showing ${count(matches.length)} of ${plural(rows.length, "item")} · ${formatSize(size)}`;
    list.show(matches);
  };

  let timer;
  search.addEventListener("input", () => {
    clearTimeout(timer);
    timer = setTimeout(refilter, SEARCH_DELAY);
  });
  extension.addEventListener("change", refilter);
  minSize.addEventListener("change", refilter);
  refilter();
}

/** One option per extension, most common first, with its count. */
function extensionOptions(rows) {
  const counts = new Map();
  for (const row of rows) counts.set(row.extension, (counts.get(row.extension) ?? 0) + 1);
  return [...counts]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([ext, n]) => el("option", { value: ext }, `.${ext} (${count(n)})`));
}

/**
 * A scrolling table that builds only the rows in view. `show(rows)` swaps its contents and
 * `setSort(sort)` marks the sorted column; clicking a header calls `onSort(key)`.
 * Columns resize by dragging a header's right edge (double-click resets), and widths are remembered.
 * Clicking a row selects it and opens a menu with "Show in Finder".
 */
function virtualList(sort, onSort) {
  const widths = loadWidths();
  const heads = COLUMNS.map((column, index) => {
    const arrow = el("span", { className: "sort-arrow" });
    const button = el("button", { type: "button", className: "sort", title: column.label }, column.label, arrow);
    button.addEventListener("click", () => onSort(column.key));
    const last = index === COLUMNS.length - 1;
    const resizer = el("div", { className: last ? "resizer last" : "resizer", title: "Drag to resize, double-click to reset" });
    const cell = el("div", { role: "columnheader", className: `scan-cell ${column.className ?? ""}` }, button, resizer);
    makeResizable(resizer, cell, index);
    return { column, cell, arrow };
  });
  const head = el("div", { className: "scan-row scan-head", role: "row" }, ...heads.map(({ cell }) => cell));
  const visible = el("div", { className: "scan-window", role: "rowgroup" });
  const body = el("div", { className: "scan-body" }, visible);
  const empty = el("p", { className: "scan-empty" }, "No items match.");
  const measure = el("div", { className: "scan-measure path", ariaHidden: "true" });
  const viewport = el("div", { className: "scan-viewport" }, head, body, empty, measure);
  const guide = el("div", { className: "resize-guide", hidden: true });
  const menu = el("div", { className: "row-menu", role: "menu", hidden: true });
  const node = el("div", { className: "scan-table", role: "table" }, viewport, guide, menu);

  let rows = [];
  let drawn = null; // [first, last) rows currently built
  let frame = 0;
  let selected = null; // path of the row whose menu is open
  const fitted = new Map(); // "width\npath" → the path as shown in a cell that wide

  /** `path`, with folders dropped from its start until it fits in two lines of a path cell. */
  const fitPath = (path) => {
    const key = `${measure.style.width}\n${path}`;
    if (!fitted.has(key)) {
      if (fitted.size > 20000) fitted.clear();
      fitted.set(
        key,
        fitTail(path, pathSeparator, (text) => {
          measure.replaceChildren(...pathNodes(text));
          return measure.offsetHeight <= 2 * LINE_HEIGHT;
        }),
      );
    }
    return fitted.get(key);
  };

  const draw = () => {
    frame = 0;
    const first = Math.max(0, Math.floor(viewport.scrollTop / ROW_HEIGHT) - OVERSCAN);
    const last = Math.min(rows.length, Math.ceil((viewport.scrollTop + viewport.clientHeight) / ROW_HEIGHT) + OVERSCAN);
    if (drawn && drawn[0] === first && drawn[1] === last) return;
    drawn = [first, last];
    measure.style.width = `${Math.max(0, heads[PATH_COLUMN].cell.getBoundingClientRect().width - CELL_PADDING)}px`;
    visible.style.transform = `translateY(${first * ROW_HEIGHT}px)`;
    visible.replaceChildren(
      ...rows.slice(first, last).map((row, i) => rowNode(row, first + i, row.entry.path === selected, fitPath)),
    );
  };
  const redraw = () => {
    drawn = null;
    if (!frame) frame = requestAnimationFrame(draw);
  };

  const closeMenu = () => {
    if (menu.hidden) return;
    menu.hidden = true;
    selected = null;
    for (const row of visible.querySelectorAll(".selected")) row.classList.remove("selected");
    document.removeEventListener("pointerdown", closeOutside, true);
    document.removeEventListener("keydown", closeOnEscape);
  };
  const closeOutside = (event) => {
    if (!menu.contains(event.target)) closeMenu();
  };
  const closeOnEscape = (event) => {
    if (event.key === "Escape") closeMenu();
  };

  visible.addEventListener("click", (event) => {
    const rowNode = event.target.closest(".scan-row");
    if (!rowNode) return;
    const { entry } = rows[Number(rowNode.dataset.index)];
    selected = entry.path;
    rowNode.classList.add("selected");

    const reveal = el("button", { type: "button", role: "menuitem" }, "Show in Finder");
    reveal.addEventListener("click", () =>
      revealInFinder(entry.path).then(closeMenu, (error) =>
        menu.replaceChildren(el("p", { className: "status error" }, describeError(error))),
      ),
    );
    menu.replaceChildren(reveal);
    menu.hidden = false;
    // Open at the pointer, kept inside the table.
    const box = node.getBoundingClientRect();
    menu.style.left = `${Math.max(0, Math.min(event.clientX - box.left, box.width - menu.offsetWidth - 4))}px`;
    menu.style.top = `${Math.max(0, Math.min(event.clientY - box.top, box.height - menu.offsetHeight - 4))}px`;
    document.addEventListener("pointerdown", closeOutside, true);
    document.addEventListener("keydown", closeOnEscape);
    reveal.focus();
  });

  viewport.addEventListener("scroll", () => {
    closeMenu();
    if (!frame) frame = requestAnimationFrame(draw);
  });
  new ResizeObserver(redraw).observe(viewport);

  function applyWidths() {
    const tracks = COLUMNS.map((column, i) => (widths[i] ? `${widths[i]}px` : column.track));
    const minTableWidth = COLUMNS.reduce((sum, column, i) => sum + (widths[i] ?? column.min), 0);
    node.style.setProperty("--scan-columns", tracks.join(" "));
    node.style.setProperty("--scan-min-width", `${minTableWidth}px`);
  }

  function makeResizable(resizer, cell, index) {
    resizer.addEventListener("click", (event) => event.stopPropagation());
    resizer.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      resizer.setPointerCapture(event.pointerId);
      // Pin every column at its current width, so only the dragged edge moves.
      heads.forEach(({ cell }, i) => (widths[i] = Math.round(cell.getBoundingClientRect().width)));
      const startX = event.clientX;
      const startWidth = widths[index];
      // A line down the whole table shows where the column will end.
      const showGuide = () => {
        guide.style.left = `${cell.getBoundingClientRect().right - node.getBoundingClientRect().left - node.clientLeft}px`;
      };
      const move = (moved) => {
        widths[index] = Math.max(MIN_COLUMN_WIDTH, Math.round(startWidth + moved.clientX - startX));
        applyWidths();
        showGuide();
        redraw();
      };
      resizer.classList.add("dragging");
      guide.hidden = false;
      showGuide();
      const stop = () => {
        resizer.classList.remove("dragging");
        guide.hidden = true;
        resizer.removeEventListener("pointermove", move);
        resizer.removeEventListener("pointerup", stop);
        resizer.removeEventListener("pointercancel", stop);
        saveWidths(widths);
      };
      resizer.addEventListener("pointermove", move);
      resizer.addEventListener("pointerup", stop);
      resizer.addEventListener("pointercancel", stop);
    });
    resizer.addEventListener("dblclick", () => {
      widths[index] = null;
      applyWidths();
      saveWidths(widths);
      redraw();
    });
  }

  const setSort = ({ key, direction }) => {
    for (const { column, cell, arrow } of heads) {
      const active = column.key === key;
      cell.ariaSort = active ? (direction === "asc" ? "ascending" : "descending") : "none";
      arrow.textContent = active ? (direction === "asc" ? "▲" : "▼") : "";
    }
  };

  applyWidths();
  setSort(sort);

  return {
    node,
    setSort,
    show(newRows) {
      closeMenu();
      rows = newRows;
      body.style.height = `${rows.length * ROW_HEIGHT}px`;
      empty.hidden = rows.length > 0;
      viewport.scrollTop = 0;
      drawn = null;
      draw();
    },
  };
}

function rowNode({ entry, path }, index, selected, fitPath) {
  const node = el(
    "div",
    { className: selected ? "scan-row selected" : "scan-row", role: "row" },
    el(
      "div",
      { className: "scan-cell", title: entry.name },
      entry.name,
      entry.isPackage ? el("span", { className: "pill package-pill" }, "Package") : "",
    ),
    el("div", { className: "scan-cell", title: entry.extension }, entry.extension),
    textCell(formatSize(entry.size), "number"),
    textCell(formatDate(entry.modified)),
    textCell(formatDate(entry.created)),
    el("div", { className: "scan-cell path", title: entry.path }, ...pathNodes(fitPath(path))),
  );
  node.dataset.index = index;
  return node;
}

/** A cell whose full text shows on hover, in case the column is too narrow for it. */
function textCell(text, className = "") {
  return el("div", { className: `scan-cell ${className}`, title: text }, text);
}

/** A path as text with a line-break opportunity after each separator. */
function pathNodes(path) {
  return path.split(pathSeparator).flatMap((part, i) => (i === 0 ? [part] : [pathSeparator, el("wbr"), part]));
}

/** Column widths the user dragged to, in px; `null` for a column left at its default. */
function loadWidths() {
  try {
    const saved = JSON.parse(localStorage.getItem(WIDTHS_KEY));
    if (Array.isArray(saved) && saved.length === COLUMNS.length) {
      return saved.map((width) => (Number.isFinite(width) && width >= MIN_COLUMN_WIDTH ? width : null));
    }
  } catch {
    // No saved widths, or storage is unavailable: use the defaults.
  }
  return COLUMNS.map(() => null);
}

function saveWidths(widths) {
  try {
    localStorage.setItem(WIDTHS_KEY, JSON.stringify(widths));
  } catch {
    // Storage is unavailable: widths last until the screen closes.
  }
}
