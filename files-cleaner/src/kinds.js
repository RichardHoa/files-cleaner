// How each Operation kind looks wherever it appears: its name, colour and icon.
import { el } from "./dom.js";
import { icon } from "./icons.js";

const KIND_LABELS = { trash: "Trash", move: "Move", rename: "Rename", createFolder: "Create Folder" };

/** "Create Folder" for `createFolder`. */
export function kindLabel(kind) {
  return KIND_LABELS[kind] ?? kind;
}

/** The kind's icon, in the kind's colour. */
export function kindIcon(kind) {
  return icon(kind, `kind-${kind}`);
}

/** The kind as a filled pill in its colour, with its icon. */
export function kindPill(kind) {
  return el("span", { className: `pill kind-pill kind-${kind}` }, icon(kind), kindLabel(kind));
}
