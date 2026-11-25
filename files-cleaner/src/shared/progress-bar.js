// Glides for about as long as the update took to come, so steady progress moves without pauses.
import { el } from "./dom.js";

/** In ms, so it never falls far behind. */
const MAX_GLIDE = 1500; // ms

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
