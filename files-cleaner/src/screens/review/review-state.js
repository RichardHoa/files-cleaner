// The Review screen's Plan, with no DOM and no Tauri: the Duplicate Groups, the Suggested Folders and the
// files left where they are, and the Emptied Folders. The screen reads its fields and changes them only
// through its methods, which build on the rules in review-choices.
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
  /** The Plan's Emptied Folder Operations as last worked out, and the ones ticked, by ID: all that can happen, or none. */
  emptied = [];
  emptiedTicked = new Set();
  #emptiedWanted = false;
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

  /** Sets the file at `path` of the Suggested Folder `choice` to `file`: "inFolder" or "trash". */
  setFile(choice, path, file) {
    choice.files.set(path, file);
  }

  /** Trashes every file of the Suggested Folder `choice`, or puts them all back when `trashed` is false. */
  trashAllFiles(choice, trashed = true) {
    for (const path of choice.files.keys()) choice.files.set(path, trashed ? "trash" : "inFolder");
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

  /** Merges the Suggested Folder `from` into `into`. */
  merge(from, into) {
    mergeFolder(this.choices, this.duplicates, from, into);
  }

  /** Removes the Suggested Folder `choice`, leaving its files where they are. */
  remove(choice) {
    removeFolder(this.choices, this.left, this.duplicates, choice);
  }

  /** The Emptied Folder Operations are now `operations`, ticked as the ones before were. */
  setEmptied(operations) {
    this.emptied = operations;
    this.tickEmptied(this.#emptiedWanted);
  }

  /** Ticks every Emptied Folder Operation that can happen when `ticked`, or none. */
  tickEmptied(ticked) {
    this.#emptiedWanted = ticked;
    this.emptiedTicked = new Set(ticked ? this.emptied.filter((op) => !op.error).map((op) => op.id) : []);
  }
}
