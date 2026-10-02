// Scan results: every scanned file and Package, plus what was left alone.
import { el } from "../dom.js";
import { formatDate, formatSize, plural } from "../format.js";

const COLUMNS = ["Name", "Extension", "Size", "Modified", "Created", "Path"];

export function render(root, nav, result) {
  const back = el("button", { type: "button" }, "Back");
  back.addEventListener("click", () => nav.home());

  const totalSize = result.entries.reduce((sum, entry) => sum + entry.size, 0);
  const summary = [
    plural(result.entries.length, "item"),
    formatSize(totalSize),
    `${plural(result.skipped, "Protected Path")} skipped`,
  ];
  if (result.unreadable.length > 0) summary.push(`${result.unreadable.length} couldn't be read`);

  root.append(
    el("div", { className: "actions" }, back),
    el("h1", {}, "Scan results"),
    el("p", { className: "path" }, result.root),
    el("p", { className: "lead" }, summary.join(" · ")),
    table(result),
  );

  if (result.unreadable.length > 0) {
    root.append(
      el("h2", {}, "Couldn't be read"),
      el("ul", { className: "path-list" }, ...result.unreadable.map((path) => el("li", {}, path))),
    );
  }
}

function table(result) {
  const head = el("tr", {}, ...COLUMNS.map((name) => el("th", { scope: "col" }, name)));
  const body = el("tbody");
  const rows = document.createDocumentFragment();
  for (const entry of result.entries) {
    rows.append(
      el(
        "tr",
        {},
        el("td", {}, entry.name, entry.isPackage ? el("span", { className: "tag" }, "Package") : ""),
        el("td", {}, entry.extension),
        el("td", { className: "number" }, formatSize(entry.size)),
        el("td", {}, formatDate(entry.modified)),
        el("td", {}, formatDate(entry.created)),
        el("td", { className: "path", title: entry.path }, relative(entry.path, result.root)),
      ),
    );
  }
  body.append(rows);
  return el("table", { className: "scan-table" }, el("thead", {}, head), body);
}

function relative(path, root) {
  return path.startsWith(`${root}/`) ? path.slice(root.length + 1) : path;
}
