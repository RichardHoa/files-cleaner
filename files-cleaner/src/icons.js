// Inline SVG icons, drawn on a 24×24 grid in the text colour (ADR-0003: no icon library).
const SVG = "http://www.w3.org/2000/svg";

const FOLDER = "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z";

const PATHS = {
  trash: "M4 7h16 M9 7V4.5h6V7 M6 7l1 13h10l1-13 M10 11v5 M14 11v5",
  move: "M4 12h15 M13 6l6 6-6 6",
  rename: "M4 20h4L19 9l-4-4L4 16z M13.5 6.5l4 4",
  createFolder: `${FOLDER} M12 10v6 M9 13h6`,
  folder: FOLDER,
  copies: "M8 8h11v12H8z M5 16V4h11",
  plus: "M12 5v14 M5 12h14",
  close: "M6 6l12 12 M18 6L6 18",
  settings:
    "M4 7h9 M17 7h3 M4 12h3 M11 12h9 M4 17h11 M19 17h1 M17 7a2 2 0 1 1-4 0a2 2 0 1 1 4 0 M11 12a2 2 0 1 1-4 0a2 2 0 1 1 4 0 M19 17a2 2 0 1 1-4 0a2 2 0 1 1 4 0",
};

/** The `name` icon as an `<svg>`. Decorative: say what it means in text beside it. */
export function icon(name, className = "") {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", `icon ${className}`.trim());
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG, "path");
  path.setAttribute("d", PATHS[name]);
  svg.append(path);
  return svg;
}
