// The introduction's line drawings, on a 160×100 grid, stroked in the accent colour (see `.intro-drawing`).
const SVG = "http://www.w3.org/2000/svg";

const DOC = "M0 0h14l6 6v20H0z M14 0v6h6";

/** Each drawing's strokes, `[d, transform]`. */
const DRAWINGS = {
  // Loose files above a folder.
  pick: [
    ["M50 50h20l6 6h34a4 4 0 0 1 4 4v26a4 4 0 0 1-4 4H50a4 4 0 0 1-4-4V54a4 4 0 0 1 4-4z"],
    [DOC, "translate(40 14) rotate(-12)"],
    [DOC, "translate(70 6) rotate(4)"],
    [DOC, "translate(102 12) rotate(14)"],
  ],
  // A Plan: rows, most of them ticked.
  plan: [
    ["M28 8h104a4 4 0 0 1 4 4v76a4 4 0 0 1-4 4H28a4 4 0 0 1-4-4V12a4 4 0 0 1 4-4z"],
    ["M40 18h12v12H40z M43 24l3 3 5-6 M62 24h58"],
    ["M40 44h12v12H40z M43 50l3 3 5-6 M62 50h44"],
    ["M40 70h12v12H40z M62 76h52"],
  ],
  // The Trash, with an arrow bringing things back out.
  trash: [
    ["M76 30h40 M90 30v-5h12v5 M80 30l3 52h26l3-52 M91 40v32 M101 40v32"],
    ["M70 60H52a12 12 0 0 1 0-24h14 M61 31l5 5-5 5"],
  ],
  // A file being read, and a download.
  download: [
    ["M40 18h22l10 10v50H40z M62 18v10h10 M48 40h16 M48 50h16 M48 60h10"],
    ["M106 22v36 M94 46l12 12 12-12 M88 66v12h36V66"],
  ],
};

/** The drawing `name` as an `<svg>`. Decorative: the card's sentence says what it shows. */
export function drawing(name) {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 160 100");
  svg.setAttribute("class", "intro-drawing");
  svg.setAttribute("aria-hidden", "true");
  for (const [d, transform] of DRAWINGS[name]) {
    const path = document.createElementNS(SVG, "path");
    path.setAttribute("d", d);
    if (transform) path.setAttribute("transform", transform);
    svg.append(path);
  }
  return svg;
}
