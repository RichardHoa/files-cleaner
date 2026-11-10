// What the Session screen shows: the Operations grouped by where they are now, the heading that
// sums them up, and what the pane says about one file. Operations are a Session's, with
// `relativePath` as names from the Scan Root down. Searching, sorting and paging a group's files
// are the Plan view's functions.

import { count, plural, splitPath, trashPlace } from "../../shared/format.js";
import { folderLabel } from "../../shared/plan/plan-view.js";

/** Which group each Operation state is in. */
const GROUP_OF = {
  done: "moved",
  undone: "putBack",
  skipped: "attention",
  failed: "attention",
  unrecoverable: "attention",
  started: "attention",
};

const GROUPS = ["moved", "trashed", "putBack", "attention"];

/** Whether `op` sent its file to the System Trash; an older Session's Trash went to a folder, so it counts as moved. */
const toSystemTrash = (op) => op.kind === "trash" && op.inSystemTrash;

/** The group of `op`: a carried-out Operation is under Moved, or Trashed when its file went to the System Trash. */
const groupOf = (op) => (GROUP_OF[op.state] === "moved" && toSystemTrash(op) ? "trashed" : GROUP_OF[op.state]);

/**
 * What can be done to `op`'s file from the Session screen: "undo" (Put back) once it was carried out, "redo"
 * (Move again) once it was put back, "retry" (Try again) if it was skipped or failed, or null. One stopped part-way has none: the Journal
 * can't say whether its file moved.
 */
export function actionFor(op) {
  return { done: "undo", undone: "redo", skipped: "retry", failed: "retry" }[op.state] ?? null;
}

/** Where `op`'s file is now, for Show in folder: null once it is gone for good, or while it's in the System Trash, which is the OS's to show. */
export function whereNow(op) {
  if (op.state === "done") return op.inSystemTrash ? null : op.destination;
  return op.state === "unrecoverable" ? null : op.source;
}

/** `[{ group, operations }]`, one per group that has any Operations, in a fixed order. */
export function groupByState(operations) {
  return GROUPS.map((group) => ({ group, operations: operations.filter((op) => groupOf(op) === group) })).filter(
    (g) => g.operations.length > 0,
  );
}

/** How long the Session's files can be put back: until they are emptied from the System Trash, if any went there. */
export function sessionLead(operations, separator) {
  const until = operations.some((op) => op.inSystemTrash) ? `until you empty the ${trashPlace(separator)}` : "at any time";
  return `Nothing was deleted. You can put back any file, or all of them, ${until}.`;
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
      const trashed = done.filter(toSystemTrash).length;
      const folders = done.filter((op) => op.kind === "createFolder").length;
      const moved = carriedOut - trashed - folders;
      const parts = [moved && `${count(moved)} moved`, trashed && `${count(trashed)} trashed`, folders && `${plural(folders, "folder")} created`];
      return parts.filter(Boolean).join(", ") || "Nothing done";
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
        // An older Session's Trash went to a folder, which is named like any other.
        trash: () => (op.inSystemTrash ? `In the ${trashPlace(separator)}.` : `In ${splitPath(op.destination, separator).parent}.`),
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
