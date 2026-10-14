// History: every Session, newest first, ten to a page with numbered page links.
import { describeError, listSessions } from "../api.js";
import { el } from "../dom.js";
import { count, plural } from "../format.js";
import { pageCount, pageNumbers } from "../paging.js";
import { noSessions, sessionList } from "../session-row.js";
import { undoSections } from "./summary.js";

const PER_PAGE = 10;

export function render(root, nav, page) {
  const home = el("button", { type: "button" }, "Home");
  home.addEventListener("click", () => nav.home());
  const total = el("p", { className: "lead" });
  const content = el("div");
  root.append(el("div", { className: "actions" }, home), el("h1", {}, "History"), total, content);
  show(content, total, nav, page);
}

/** Draws page `page` (from 1) into `node`, with an Undo's result above it. */
async function show(node, totalLine, nav, page, undoResult = null) {
  let result;
  try {
    result = await listSessions((page - 1) * PER_PAGE, PER_PAGE);
  } catch (error) {
    node.replaceChildren(el("p", { className: "status error" }, describeError(error)));
    return;
  }
  const pages = pageCount(result.total, PER_PAGE);
  // Sessions can't be deleted today, but a page past the end shows the last one rather than nothing.
  if (page > pages && pages > 0) return show(node, totalLine, nav, pages, undoResult);

  totalLine.textContent = result.total === 0 ? "" : `${plural(result.total, "Session")} · page ${count(page)} of ${count(pages)}`;
  const list =
    result.sessions.length > 0
      ? sessionList(result.sessions, {
          open: (id) => nav.session(id, { page }),
          undone: (undone) => show(node, totalLine, nav, page, undone),
        })
      : noSessions();
  node.replaceChildren(...(undoResult ? undoSections(undoResult) : []), list, pager(page, pages, (to) => nav.history(to)));
}

/** Previous, the page numbers and Next; nothing for a single page. */
function pager(page, pages, go) {
  if (pages <= 1) return "";
  const link = (label, to, ariaLabel) => {
    const button = el("button", { type: "button", className: "page-link" }, label);
    if (ariaLabel) button.ariaLabel = ariaLabel;
    button.disabled = to < 1 || to > pages;
    button.addEventListener("click", () => go(to));
    return button;
  };
  const numbers = pageNumbers(page, pages).map((n) => {
    if (n === null) return el("span", { className: "page-gap" }, "…");
    const button = link(count(n), n, `Page ${n}`);
    if (n === page) {
      button.classList.add("current");
      button.ariaCurrent = "page";
    }
    return button;
  });
  return el("nav", { className: "pager", ariaLabel: "Pages" }, link("‹ Previous", page - 1), ...numbers, link("Next ›", page + 1));
}
