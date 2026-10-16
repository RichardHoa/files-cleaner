// What the Session screen shows: the Operations grouped by where they are now, the heading that
// sums them up, and what the pane says about one file. Operations are a Session's, with
// `relativePath` and `relativeDestination` as names from the Scan Root down. Searching, sorting
// and paging a group's files are the Plan view's functions.

import { count, plural, splitPath } from "./format.js";
import { folderLabel } from "./plan-view.js";

/** Which group each Operation state is in. */
const GROUP_OF = {
  done: "moved",
  undone: "putBack",
  skipped: "attention",
  failed: "attention",
  unrecoverable: "attention",
  started: "attention",
};

const GROUPS = ["moved", "putBack", "attention"];

/** `[{ group, operations }]`, one per group that has any Operations, in a fixed order. */
export function groupByState(operations) {
  return GROUPS.map((group) => ({ group, operations: operations.filter((op) => GROUP_OF[op.state] === group) })).filter(
    (g) => g.operations.length > 0,
  );
}

/** What the Holding Folder is called, from where the Session put the files it trashed. */
export function holdingName(operations) {
  return operations.find((op) => op.kind === "trash")?.relativeDestination[0] ?? "Files Cleaner Trash";
}

/**
 * What the Session did, as things are now, given its Operations and its status. Counts the
 * Operations that were carried out; one that stopped part-way counts every Operation it started.
 */
export function sessionHeading(operations, status) {
  const carriedOut = operations.filter((op) => ["done", "undone", "unrecoverable"].includes(op.state)).length;
  const putBack = operations.filter((op) => op.state === "undone").length;
  const files = (n) => plural(n, "file");
  switch (status) {
    case "undone":
      return `All ${files(carriedOut)} ${carriedOut === 1 ? "is" : "are"} back`;
    case "partlyUndone":
      return `${count(putBack)} of ${files(carriedOut)} put back`;
    case "crashed":
      return `Stopped after ${count(carriedOut)} of ${files(operations.length)}`;
    default: {
      const done = operations.filter((op) => ["done", "undone", "unrecoverable"].includes(op.state));
      if (done.every((op) => op.kind === "trash")) return `${files(carriedOut)} moved to ${holdingName(operations)}`;
      const folders = done.filter((op) => op.kind === "createFolder").length;
      const moved = `${files(carriedOut - folders)} moved`;
      return folders ? `${moved}, ${plural(folders, "folder")} created` : moved;
    }
  }
}

/** How the pane starts its sentence about a file under Needs attention. */
const ATTENTION_LEADS = {
  skipped: "Skipped",
  failed: "Failed",
  unrecoverable: "Couldn't be recovered",
  started: "Stopped before this file finished",
};

/**
 * What the pane says about `op`: where it is now, or, under Needs attention, what happened to it.
 * Its `note` says why the last pass over it didn't happen. `rootName` is the Scan Root's name, and
 * `separator` the OS's path separator.
 */
export function paneSentence(op, rootName, separator) {
  const note = op.note;
  switch (GROUP_OF[op.state]) {
    case "moved": {
      const where = {
        trash: () => `In ${op.relativeDestination[0]}, inside ${rootName}.`,
        move: () => `In ${splitPath(op.destination, separator).parent}.`,
        createFolder: () => "Created by this Session.",
      }[op.kind]();
      return note ? `${where} Not put back: ${note}` : where;
    }
    case "putBack": {
      const where = op.kind === "createFolder" ? "Removed again." : `Back in ${folderLabel(op, rootName)}.`;
      return note ? `${where} Not moved again: ${note}` : where;
    }
    default: {
      const lead = ATTENTION_LEADS[op.state];
      return note ? `${lead}: ${note}` : `${lead}.`;
    }
  }
}
