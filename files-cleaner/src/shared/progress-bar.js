// A progress bar that glides between updates instead of jumping. Progress arrives every so often (each
// file read, or every 100–250 ms), so the bar takes about as long to reach a new value as that value took
// to come, which keeps steady progress moving without pauses.
import { el } from "./dom.js";

/** The longest the bar takes to reach a new value, so it never falls far behind. */
const MAX_GLIDE = 1500; // ms

/** `{ node, set(value, max) }`: a bar, moving back and forth until `set` is given a `max` above 0. */
export function progressBar(className = "") {
  const fill = el("div", { className: "progress-fill" });
  const node = el("div", { className: `progress indeterminate ${className}`.trim(), role: "progressbar" }, fill);
  node.ariaValueMin = "0";
  node.ariaValueMax = "100";
  let last = null;

  return {
    node,
    set(value, max) {
      if (!(max > 0)) {
        node.classList.add("indeterminate");
        node.removeAttribute("aria-valuenow");
        last = null;
        return;
      }
      const fraction = Math.min(Math.max(value / max, 0), 1);
      const now = performance.now();
      // A bar starting out, or going back for a new phase, jumps there.
      const glide = last === null || fraction < last.fraction ? 0 : Math.min(now - last.at, MAX_GLIDE);
      node.classList.remove("indeterminate");
      fill.style.transitionDuration = `${glide}ms`;
      fill.style.transform = `scaleX(${fraction})`;
      node.ariaValueNow = String(Math.round(fraction * 100));
      last = { fraction, at: now };
    },
  };
}
