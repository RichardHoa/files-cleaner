// The Review screen's Plan, with no DOM and no Tauri: the Duplicate Groups, the Suggested Folders and the
// files left where they are, and the Emptied Folders. The screen reads its fields and changes them only
// through its methods, which build on the rules in review-choices.
import {
  addFolder,
  filedCopy,
  filedCopyTrashed,
  initialChoices,
  initialDuplicates,
  leaveUnsorted,
  LeftFiles,
  markedTrash,
  mergeFolder,
  moveFile,
  moveToOtherFolder,
  placeDuplicates,
  setCopy,
} from "./review-choices.js";

export class ReviewState {
  /**
   * The Suggested Folders' phase: "reading", "offer" (without the Grouping Models), "ask" (reading would take
   * long, so it waits for the user to say so), "failed" or "ready", and
   * once they're ready, the grouping they came from.
   */
  phase = "reading";
  /** Once the Rules' folders are shown while the rest of the files are read: "reading", then "failed" or null once they're in. */
  more = null;
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

  /** The Suggested Folders' phase is now `phase`, before they're ready; once the Rules' folders are shown, it's the rest's. */
  waitFor(phase) {
    if (this.phase === "ready") this.more = phase;
    else this.phase = phase;
  }

  /** Fills in the Rules' Suggested Folders from `grouping`, the rest of the files left where they are while they're read. */
  showRuled(grouping) {
    this.ready(grouping);
    this.more = "reading";
  }

  /**
   * Fills in the Suggested Folders and the files left where they are from `grouping`, with the choices made on
   * the copies meanwhile, and returns the folders added. Once the Rules' folders are shown, it only adds to them.
   */
  ready(grouping) {
    if (this.phase === "ready") return this.#addRest(grouping);
    this.grouping = grouping;
    this.choices = initialChoices(grouping.folders);
    for (const path of grouping.ungrouped) this.left.add(path);
    placeDuplicates(this.choices, this.left, this.duplicates);
    this.phase = "ready";
    return this.choices;
  }

  /**
   * Adds the folders `grouping` made of the files read to those of the Rules, shown and maybe changed while
   * they were read: each folder takes its files that are still left where they are and not trashed, and a
   * Rule's folder takes the files the tool placed in it, unless the user removed it. Returns the folders added.
   */
  #addRest(grouping) {
    this.grouping = grouping;
    this.more = null;
    // The tool lists each Duplicate Group by its first copy; its place is held by its Filed Copy.
    const take = (path, choice) => {
      const duplicate = this.duplicateOf.get(path);
      if (duplicate && !duplicate.placed) {
        if (duplicate.emptyPlace?.choice !== null) return false;
        duplicate.emptyPlace = { choice, file: "inFolder" };
        return true;
      }
      const placed = duplicate ? duplicate.placed : path;
      if (!this.left.has(placed) || this.left.trashed.has(placed)) return false;
      this.left.delete(placed);
      choice.files.set(placed, "inFolder");
      return true;
    };
    const added = [];
    for (const folder of grouping.folders) {
      if (folder.basis === "rule") {
        const name = folder.name.toLowerCase();
        const choice = this.choices.find((c) => c.folder.rule && c.folder.name.toLowerCase() === name);
        for (const path of choice ? folder.files : []) take(path, choice);
        continue;
      }
      const [choice] = initialChoices([folder]);
      choice.files.clear();
      if (folder.files.map((path) => take(path, choice)).includes(true)) added.push(choice);
    }
    this.choices.push(...added);
    return added;
  }

  /** Sets the copy at `path` of the Duplicate Group `duplicate` to `fate`, "keep" or "trash". */
  setCopy(duplicate, path, fate) {
    setCopy(duplicate, path, fate, this.choices, this.left);
  }

  /**
   * The copies at `paths` went to the System Trash on their own, before the rest of the Plan: drops them from
   * their Duplicate Groups, then every group left with one copy or none, whose Kept Copy is now a file like any other.
   */
  copiesTrashed(paths) {
    const gone = new Set(paths);
    for (const duplicate of this.duplicates) {
      for (const path of gone) duplicate.copies.delete(path);
      duplicate.group = { ...duplicate.group, copies: duplicate.group.copies.filter((copy) => !gone.has(copy.path)) };
    }
    this.duplicates = this.duplicates.filter((duplicate) => duplicate.copies.size > 1);
  }

  /**
   * Trash Now of the Suggested Folders `chosen`: `folders`, every file listed in them trashed, as
   * `buildReviewPlan` takes them, so no folder is made, and `trashed(paths)`, once the files at `paths` went to
   * the System Trash, dropping them and each folder left with none.
   */
  trashNow(chosen) {
    const folders = chosen
      .filter((choice) => choice.files.size > 0)
      .map((choice) => ({ name: choice.name.trim() || "No name", choice: { type: "trashAll" }, files: [...choice.files.keys()].map((path) => [path, "trash"]) }));
    const trashed = (paths) => {
      const gone = new Set(paths);
      for (const choice of chosen) {
        for (const path of gone) choice.files.delete(path);
        if (choice.files.size === 0 && this.choices.includes(choice)) this.leaveUnsorted(choice);
      }
      for (const duplicate of this.duplicates) {
        if (gone.has(filedCopy(duplicate))) filedCopyTrashed(duplicate, this.choices, this.left);
      }
      // A group left with one copy is a file like any other.
      this.duplicates = this.duplicates.filter((duplicate) => duplicate.copies.size > 1);
    };
    return { folders, trashed };
  }

  /** Sets the file at `path` of the Suggested Folder `choice` to `file`: "inFolder" or "trash". */
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

  /** Moves `paths` into the Suggested Folder `to`, or leaves them where they are when `to` is null; one marked Trash stays where it is. */
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

  /** Leaves the files of the Suggested Folder `choice` not marked Trash where they are; it's dropped once it has none. */
  leaveUnsorted(choice) {
    leaveUnsorted(this.choices, this.left, this.duplicates, choice);
  }

  /** How many of `paths` aren't marked Trash, so Move takes them. */
  movable(paths) {
    return paths.filter((path) => !markedTrash(this.choices, this.left, path)).length;
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
