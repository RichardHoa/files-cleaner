// Small pieces the Review screen's panes share, with no state of their own. The panes reach each other
// only through the Review screen's context, never by importing one another.
import { pathSeparator } from "../../shared/api.js";
import { count, splitPath } from "../../shared/format.js";

/** How many of a card's files, or Duplicate Groups, are listed before "Show more". */
export const FILES_SHOWN = 50;

/** The file or folder's own name, from its full `path`. */
export const fileName = (path) => splitPath(path, pathSeparator).name;

/** The Suggested Folder `choice`'s name as listed, or "No name" while it has none. */
export const displayName = (choice) => choice.name.trim() || "No name";

/** `n` copies, as "1 copy" or "3 copies". */
export const copiesText = (n) => `${count(n)} ${n === 1 ? "copy" : "copies"}`;
