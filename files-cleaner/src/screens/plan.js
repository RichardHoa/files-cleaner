// Plan: the proposed Operations as a before/after tree. The user ticks or unticks each
// Operation or whole folders, then runs the ticked ones or cancels. Nothing changes until Run.
import { runPlan } from "../api.js";
import { el } from "../dom.js";
import { formatSize, plural } from "../format.js";
import { kindLabel, kindPill } from "../kinds.js";
import { folderState, toggleFolder, totals } from "../plan-selection.js";
import { busy } from "../status.js";

export function render(root, nav, result, plan) {
  const byId = new Map(plan.operations.map((op) => [op.id, op]));
  let ticked = new Set(byId.keys());

  const cancel = el("button", { type: "button" }, "Cancel");
  cancel.addEventListener("click", () => nav.scanResults(result));
  const run = el("button", { type: "button", className: "primary" }, "Run");
  const totalsLine = el("p", { className: "lead" });
  const status = el("p", { className: "status" });

  root.append(el("h1", {}, "Plan"), el("p", { className: "path" }, plan.root));
  if (plan.operations.length === 0) {
    root.append(el("p", { className: "lead" }, "Nothing to do: no duplicates were found."), el("div", { className: "actions" }, cancel));
    return;
  }

  // Every tick box and after-tree item, so a change can refresh them all.
  const folderBoxes = [];
  const itemBoxes = [];
  const afterItems = [];

  const refresh = () => {
    for (const { node, box } of folderBoxes) {
      const state = folderState(node, ticked);
      box.checked = state === "all";
      box.indeterminate = state === "some";
    }
    for (const { id, box } of itemBoxes) box.checked = ticked.has(id);
    for (const { id, item } of afterItems) item.classList.toggle("unticked", !ticked.has(id));
    const { count, byKind, trashBytes } = totals(plan.operations, ticked);
    const kinds = Object.entries(byKind).map(([kind, n]) => `${kindLabel(kind)}: ${n}`);
    totalsLine.textContent = [
      `${plural(count, "Operation")} ticked of ${plan.operations.length}`,
      ...kinds,
      `${formatSize(trashBytes)} to trash`,
    ].join(" · ");
    run.disabled = count === 0;
  };
  const setTicked = (next) => {
    ticked = next;
    refresh();
  };

  const beforeTree = (node) => {
    if (node.operationId) {
      const op = byId.get(node.operationId);
      const box = el("input", { type: "checkbox" });
      box.addEventListener("change", () => {
        const next = new Set(ticked);
        if (box.checked) next.add(op.id);
        else next.delete(op.id);
        setTicked(next);
      });
      itemBoxes.push({ id: op.id, box });
      return el(
        "li",
        {},
        el(
          "label",
          { className: "tree-item" },
          box,
          el("span", {}, node.name),
          kindPill(op.kind),
          el("span", { className: "muted" }, formatSize(op.size)),
        ),
        el("div", { className: "reason" }, op.reason),
      );
    }
    const box = el("input", { type: "checkbox" });
    box.addEventListener("change", () => setTicked(toggleFolder(node, ticked)));
    folderBoxes.push({ node, box });
    return folder(node, box, beforeTree);
  };

  const afterTree = (node) => {
    if (node.operationId) {
      const item = el("li", { className: "tree-item", title: "Stays where it is while unticked" }, node.name);
      afterItems.push({ id: node.operationId, item });
      return item;
    }
    return folder(node, null, afterTree);
  };

  run.addEventListener("click", async () => {
    const session = await busy(status, [run, cancel], `Running ${plural(ticked.size, "Operation")}…`, () => runPlan([...ticked]));
    if (session) nav.summary({ session });
  });

  root.append(
    totalsLine,
    el("div", { className: "actions" }, run, cancel),
    status,
    el(
      "div",
      { className: "plan-trees" },
      el("section", {}, el("h2", {}, "Before"), el("ul", { className: "tree" }, beforeTree(plan.before))),
      el("section", {}, el("h2", {}, "After"), el("ul", { className: "tree" }, afterTree(plan.after))),
    ),
  );
  refresh();
}

/** A folder in a tree, open, with an optional tick box and its children drawn by `child`. */
function folder(node, box, child) {
  // A tick box is interactive content, so clicking it doesn't open or close the folder.
  const label = el("span", { className: "tree-folder" }, ...(box ? [box] : []), `${node.name}/`);
  return el("li", {}, el("details", { open: true }, el("summary", {}, label), el("ul", {}, ...node.children.map(child))));
}
