// Groups by where files are now, not by what the Session did, since Undo and Redo change that.

import { count, plural, splitPath, trashPlace } from "../../shared/format.js";
import { folderLabel } from "../../shared/plan/plan-view.js";

const GROUP_OF = {
  done: "moved",
  undone: "putBack",
  skipped: "attention",
  failed: "attention",
  unrecoverable: "attention",
  started: "attention",
};

const GROUPS = ["moved", "trashed", "putBack", "attention"];

/** An older Session's Trash went to a folder, so it counts as moved. */
const toSystemTrash = (op) => op.kind === "trash" && op.inSystemTrash;

const groupOf = (op) => (GROUP_OF[op.state] === "moved" && toSystemTrash(op) ? "trashed" : GROUP_OF[op.state]);

/** None for one stopped part-way: the Journal can't say whether its file moved. */
export function actionFor(op) {
  return { done: "undo", undone: "redo", skipped: "retry", failed: "retry" }[op.state] ?? null;
}

/** Null in the System Trash, which is the OS's to show, or once it's gone. */
export function whereNow(op) {
  if (op.state === "done") return op.inSystemTrash ? null : op.destination;
  return op.state === "unrecoverable" ? null : op.source;
}

export function groupByState(operations) {
  return GROUPS.map((group) => ({ group, operations: operations.filter((op) => groupOf(op) === group) })).filter(
    (g) => g.operations.length > 0,
  );
}

/** Only until the System Trash is emptied, if any went there. */
export function sessionLead(operations, separator) {
  return operations.some((op) => op.inSystemTrash) ? `Undo anytime until the ${trashPlace(separator)} is emptied` : "Undo anytime";
}

/** One that stopped part-way counts every Operation it started. */
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

const ATTENTION_LEADS = {
  skipped: "Skipped",
  failed: "Failed",
  unrecoverable: "Couldn't be recovered",
  started: "Stopped before this file finished",
};

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
