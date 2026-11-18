// The Review screen's Plan, with no DOM and no Tauri: the Duplicate Groups, the Suggested Folders and the
// files left where they are, what the parts run on their own did, and the Emptied Folders. The screen reads
// its fields and changes them only through its methods, which build on the rules in review-choices.
import {
  addFolder,
  initialChoices,
  initialDuplicates,
  keepCopyIn,
  LeftFiles,
  mergeFolder,
  moveFile,
  moveToOtherFolder,
  placeDuplicates,
  removeFolder,
  setCopy,
} from "./review-choices.js";

export class ReviewState {
  /**
   * The Suggested Folders' phase: "reading", "offer" (without the Grouping Models), "failed" or "ready", and
   * once they're ready, the grouping they came from.
   */
  phase = "reading";
  grouping = null;
  choices = [];
  left = new LeftFiles();
  /**
   * Every file a part run on its own moved or trashed; the rest of the Plan is built with them, only so the
   * folders they leave empty are offered. A file skipped or failed isn't: it can be run again.
   */
  ran = new Set();
  /** What each run of the trashes of a Suggested Folder, or of the files left where they are ("left"), did, by the folder. */
  trashesDone = new Map();
  /** What trashing the copies did, once it's done. */
  duplicatesDone = null;
  /** The Plan's Emptied Folder Operations as last worked out, and the ones ticked, by ID. */
  emptied = [];
  emptiedTicked = new Set();
  #sizes;
  #modified;
  #readAs;

  /** The Plan of the scan `result` and its `duplicateGroups`, with the Suggested Folders still being read. */
  constructor(result, duplicateGroups) {
    this.#sizes = new Map(result.entries.map((entry) => [entry.path, entry.size]));
    this.#modified = new Map(result.entries.map((entry) => [entry.path, entry.modified]));
    this.duplicates = initialDuplicates(duplicateGroups);
    /** The Duplicate Group each copy is in, by path. */
    this.duplicateOf = new Map(this.duplicates.flatMap((duplicate) => duplicate.group.copies.map((copy) => [copy.path, duplicate])));
    // The copy read for each copy in a Duplicate Group, its first, so any Kept Copy shows what was read.
    this.#readAs = new Map(duplicateGroups.flatMap((group) => group.copies.map((copy) => [copy.path, group.copies[0].path])));
  }

  /** The one-sentence summary of the file at `path`, through the copy read, if it was read. */
  summaryOf(path) {
    return this.grouping?.summaries[this.#readAs.get(path) ?? path];
  }

  /** The kind of the file at `path`, through the copy read, if it was read. */
  kindOf(path) {
    return this.grouping?.kinds[this.#readAs.get(path) ?? path];
  }

  /** The size of the file at `path`, in bytes. */
  sizeOf(path) {
    return this.#sizes.get(path);
  }

  /** When the file at `path` was last changed. */
  modifiedOf(path) {
    return this.#modified.get(path);
  }

  /** Whether the Duplicate Group `duplicate` has a copy already run, by a Suggested Folder or the files left where they are: it can't change any more. */
  locked(duplicate) {
    return duplicate.group.copies.some((copy) => this.ran.has(copy.path));
  }

  /** The Suggested Folders' phase is now `phase`, before they're ready. */
  waitFor(phase) {
    this.phase = phase;
  }

  /** Fills in the Suggested Folders and the files left where they are from `grouping`, with the choices made on the copies meanwhile. */
  ready(grouping) {
    this.grouping = grouping;
    this.choices = initialChoices(grouping.folders);
    for (const path of grouping.ungrouped) this.left.add(path);
    placeDuplicates(this.choices, this.left, this.duplicates);
    this.phase = "ready";
  }

  /** Sets the copy at `path` of the Duplicate Group `duplicate` to `fate`, "keep" or "trash". */
  setCopy(duplicate, path, fate) {
    setCopy(duplicate, path, fate, this.choices, this.left);
  }

  /** Keeps the copy in `folder` of each group not changed by hand; returns how many groups changed by hand it left alone. */
  keepCopyIn(folder) {
    return keepCopyIn(this.duplicates, folder, this.choices, this.left);
  }

  /** Sets the file at `path` of the Suggested Folder `choice` to `file`: "inFolder", "trash" or "takeOut". */
  setFile(choice, path, file) {
    choice.files.set(path, file);
  }

  /** Trashes the file at `path` left where it is, or puts it back when `trashed` is false. */
  trashLeft(path, trashed) {
    if (trashed) this.left.trashed.add(path);
    else this.left.trashed.delete(path);
  }

  /** Names the Suggested Folder `choice` `name`. */
  rename(choice, name) {
    choice.name = name;
  }

  /** Sends the Suggested Folder `choice` to `value`; for "createIn", inside `elsewhere`. */
  setChoice(choice, value, elsewhere = choice.elsewhere) {
    choice.choice = value;
    choice.elsewhere = elsewhere;
  }

  /** Adds a Suggested Folder of the user's own, with no files, and returns it. */
  addFolder() {
    return addFolder(this.choices);
  }

  /** Moves `paths` into the Suggested Folder `to`, or leaves them where they are when `to` is null. */
  move(paths, to) {
    for (const path of paths) moveFile(this.choices, this.left, path, to);
  }

  /** Moves `paths` into the Suggested Folder going into `folder`, any folder on disk, and returns it, or null if they were all in it already. */
  moveToOther(paths, folder, separator) {
    return moveToOtherFolder(this.choices, this.left, paths, folder, separator);
  }

  /** Merges the Suggested Folder `from` into `into`, which takes over what its trashes did. */
  merge(from, into) {
    mergeFolder(this.choices, this.duplicates, from, into);
    this.#handOverTrashes(from, into);
  }

  /** Removes the Suggested Folder `choice`, leaving its files where they are, except those already run, which are gone. */
  remove(choice) {
    removeFolder(this.choices, this.left, this.duplicates, choice);
    for (const path of this.ran) this.left.delete(path);
    this.#handOverTrashes(choice, "left");
  }

  /** Hands what the trashes of the Suggested Folder `from` did over to `to`, a folder or "left". */
  #handOverTrashes(from, to) {
    if (this.trashesDone.has(from)) this.#addTrashesDone(to, this.trashesDone.get(from));
    this.trashesDone.delete(from);
  }

  /** Records `summary`, what trashing the copies did. */
  ranDuplicates(summary) {
    this.duplicatesDone = summary;
    this.#addRan(summary);
  }

  /** Records `summary`, what moving the files into the Suggested Folder `choice` did: it's finished, only its trashes can still run. */
  ranMoves(choice, summary) {
    this.#ranPart(summary);
    choice.moved = summary;
  }

  /** Records `summary`, what the trashes of `key`, a Suggested Folder or "left", did. */
  ranTrashes(key, summary) {
    this.#ranPart(summary);
    this.#addTrashesDone(key, [summary]);
  }

  /** Adds `summaries` to what the trashes of `key`, a Suggested Folder or "left", did. */
  #addTrashesDone(key, summaries) {
    this.trashesDone.set(key, [...(this.trashesDone.get(key) ?? []), ...summaries]);
  }

  #ranPart(summary) {
    this.#addRan(summary);
    // Left out of "Keep the copy in…" from now on.
    for (const duplicate of this.duplicates) {
      if (this.locked(duplicate)) duplicate.changedByHand = true;
    }
  }

  /** Puts the files `summary` says left their place, done or trashed for good, in `ran`. */
  #addRan(summary) {
    for (const outcome of [...summary.done, ...summary.unrecoverable]) this.ran.add(outcome.source);
  }

  /** The Emptied Folder Operations are now `operations`; a tick on one no longer there is dropped. */
  setEmptied(operations) {
    this.emptied = operations;
    const ids = new Set(operations.map((op) => op.id));
    this.emptiedTicked = new Set([...this.emptiedTicked].filter((id) => ids.has(id)));
  }

  /** Ticks the Emptied Folder Operations `ids`, and only those. */
  tickEmptied(ids) {
    this.emptiedTicked = new Set(ids);
  }
}
