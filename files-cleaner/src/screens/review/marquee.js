// Selecting rows by dragging a rectangle over them, as in Finder's list view: a drag that starts on a
// row's blank part or on empty space draws the rectangle, and the rows it touches become the selection.
// The page scrolls on its own while the pointer is near the top or bottom of the window.
import { el } from "../../shared/dom.js";

/** How far the pointer moves before a press becomes a drag, so a click stays a click. */
const DRAG_THRESHOLD = 4;
/** How near the window's top or bottom edge the page starts scrolling, and how far it scrolls per frame. */
const SCROLL_EDGE = 40;
const SCROLL_STEP = 12;

/**
 * Lets the user drag-select the rows of `container`. `rows()` is every selectable row now listed, each
 * with `dataset.path`; a press on a control, or wherever `ignore(target)` is true (such as a file's name,
 * which drags the file instead), is left alone. `select(paths)` replaces the selection with the rows the
 * rectangle touches, and `clear()` runs on a plain click on empty space, not on a row, a control or
 * wherever `ignoreClick(target)` is true.
 */
export function marqueeSelect(container, { rows, ignore, ignoreClick = () => false, select, clear }) {
  const box = el("div", { className: "marquee", hidden: true });
  let start = null; // { x, y } in page coordinates while the button is down
  let pointer = null; // the last { clientX, clientY }
  let dragging = false;
  let frame = 0;

  /** Whether `target` is in a control, which handles the pointer itself. */
  const isControl = (target) => target.closest("button, input, select, a, label, [contenteditable]");

  /** The rectangle from the press to the pointer, in page coordinates. */
  function rect() {
    const x = pointer.clientX + window.scrollX;
    const y = pointer.clientY + window.scrollY;
    return { left: Math.min(start.x, x), right: Math.max(start.x, x), top: Math.min(start.y, y), bottom: Math.max(start.y, y) };
  }

  /** Draws the rectangle and selects the rows it touches. */
  function draw() {
    const r = rect();
    const origin = container.getBoundingClientRect();
    box.style.left = `${r.left - origin.left - window.scrollX}px`;
    box.style.top = `${r.top - origin.top - window.scrollY}px`;
    box.style.width = `${r.right - r.left}px`;
    box.style.height = `${r.bottom - r.top}px`;
    const touched = rows().filter((row) => {
      const b = row.getBoundingClientRect();
      const top = b.top + window.scrollY;
      const left = b.left + window.scrollX;
      return top < r.bottom && top + b.height > r.top && left < r.right && left + b.width > r.left;
    });
    select(touched.map((row) => row.dataset.path));
  }

  /** Scrolls the page while the pointer is near the window's edge, drawing the rectangle as it grows. */
  function scroll() {
    frame = 0;
    if (!dragging) return;
    const step = pointer.clientY < SCROLL_EDGE ? -SCROLL_STEP : pointer.clientY > window.innerHeight - SCROLL_EDGE ? SCROLL_STEP : 0;
    if (step === 0) return;
    window.scrollBy(0, step);
    draw();
    frame = requestAnimationFrame(scroll);
  }

  container.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || isControl(event.target) || ignore(event.target)) return;
    start = { x: event.clientX + window.scrollX, y: event.clientY + window.scrollY };
    pointer = event;
    dragging = false;
  });
  container.addEventListener("pointermove", (event) => {
    if (!start) return;
    pointer = event;
    if (!dragging) {
      if (Math.hypot(event.clientX + window.scrollX - start.x, event.clientY + window.scrollY - start.y) < DRAG_THRESHOLD) return;
      dragging = true;
      container.setPointerCapture(event.pointerId);
      container.append(box);
      box.hidden = false;
    }
    draw();
    if (!frame) frame = requestAnimationFrame(scroll);
  });
  /** Ends the press, and the drag if it was one. */
  const end = () => {
    start = null;
    if (!dragging) return;
    box.hidden = true;
    box.remove();
    cancelAnimationFrame(frame);
    frame = 0;
    // The click that follows the drag would select the one row under the pointer. It comes in the same
    // task as the pointerup, if at all, as it doesn't when the button was let go outside the window.
    container.addEventListener("click", swallow, true);
    setTimeout(() => {
      container.removeEventListener("click", swallow, true);
      dragging = false;
    });
  };
  /** Stops the click that ends a drag from reaching the row under the pointer. */
  const swallow = (event) => event.stopPropagation();
  container.addEventListener("pointerup", end);
  container.addEventListener("pointercancel", end);
  // A plain click on empty space, not on any row, deselects.
  container.addEventListener("click", (event) => {
    if (dragging || isControl(event.target) || ignoreClick(event.target) || rows().some((row) => row.contains(event.target))) return;
    clear();
  });
}
