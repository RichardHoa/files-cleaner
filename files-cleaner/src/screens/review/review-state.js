// No DOM and no Tauri, so it can be tested. The screen changes it only through its methods.
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
  /** "ask" means reading would take long, so it waits for the user. */
  phase = "reading";
  /** Null unless the Rules' folders are shown while the rest is still read, or failed to be. */
  more = null;
  grouping = null;
  choices = [];
  left = new LeftFiles();
  emptied = [];
  emptiedTicked = new Set();
  #emptiedWanted = false;
  #sizes;
  #modified;
  #readAs;

  constructor(result, duplicateGroups) {
    this.#sizes = new Map(result.entries.map((entry) => [entry.path, entry.size]));
    this.#modified = new Map(result.entries.map((entry) => [entry.path, entry.modified]));
    this.duplicates = initialDuplicates(duplicateGroups);
    this.duplicateOf = new Map(this.duplicates.flatMap((duplicate) => duplicate.group.copies.map((copy) => [copy.path, duplicate])));
    // Only the first copy is read, so any other is looked up through it.
    this.#readAs = new Map(duplicateGroups.flatMap((group) => group.copies.map((copy) => [copy.path, group.copies[0].path])));
  }

  summaryOf(path) {
    return this.grouping?.summaries[this.#readAs.get(path) ?? path];
  }

  kindOf(path) {
    return this.grouping?.kinds[this.#readAs.get(path) ?? path];
  }

  sizeOf(path) {
    return this.#sizes.get(path);
  }

  modifiedOf(path) {
    return this.#modified.get(path);
  }

  waitFor(phase) {
    if (this.phase === "ready") this.more = phase;
    else this.phase = phase;
  }

  showRuled(grouping) {
    this.ready(grouping);
    this.more = "reading";
  }

  /** Once the Rules' folders are shown it only adds to them, as the user may have changed them. */
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
   * The user may have changed the Rules' folders meanwhile, so only files still left unsorted and
   * not trashed are added.
   */
  #addRest(grouping) {
    this.grouping = grouping;
    this.more = null;
    // The tool lists a group by its first copy, but its Filed Copy may be another by now.
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

  setCopy(duplicate, path, fate) {
    setCopy(duplicate, path, fate, this.choices, this.left);
  }

  /** A group left with one copy is dropped: that copy is now a file like any other. */
  copiesTrashed(paths) {
    const gone = new Set(paths);
    for (const duplicate of this.duplicates) {
      for (const path of gone) duplicate.copies.delete(path);
      duplicate.group = { ...duplicate.group, copies: duplicate.group.copies.filter((copy) => !gone.has(copy.path)) };
    }
    this.duplicates = this.duplicates.filter((duplicate) => duplicate.copies.size > 1);
  }

  /** Planned as "trash all", so no folder is made. */
  trashNow(chosen) {
    const folders = chosen
      .filter((choice) => choice.files.size > 0)
      .map((choice) => ({ name: choice.name.trim() || "No name", choice: { type: "trashAll" }, files: [...choice.files.keys()].map((path) => [path, "trash"]) }));
    return { folders, trashed: (paths) => this.gone(paths) };
  }

  /** A Session moved or trashed these and they stayed so: they're no longer where the Review has them. */
  gone(paths) {
    const gone = new Set(paths);
    for (const path of gone) this.left.delete(path);
    for (const choice of [...this.choices]) {
      const had = [...gone].filter((path) => choice.files.delete(path)).length > 0;
      if (had && choice.files.size === 0) this.leaveUnsorted(choice);
    }
    // The other copies first, so the one kept in a Filed Copy's place is one still there.
    const filedGone = this.duplicates.filter((duplicate) => gone.has(filedCopy(duplicate)));
    const filed = new Set(filedGone.map(filedCopy));
    this.copiesTrashed([...gone].filter((path) => !filed.has(path)));
    for (const duplicate of filedGone) {
      if (this.duplicates.includes(duplicate)) filedCopyTrashed(duplicate, this.choices, this.left);
    }
    this.duplicates = this.duplicates.filter((duplicate) => duplicate.copies.size > 1);
  }

  setFile(choice, path, file) {
    choice.files.set(path, file);
  }

  trashLeft(path, trashed) {
    if (trashed) this.left.trashed.add(path);
    else this.left.trashed.delete(path);
  }

  rename(choice, name) {
    choice.name = name;
  }

  setChoice(choice, value, elsewhere = choice.elsewhere) {
    choice.choice = value;
    choice.elsewhere = elsewhere;
  }

  addFolder() {
    return addFolder(this.choices);
  }

  move(paths, to) {
    for (const path of paths) moveFile(this.choices, this.left, path, to);
  }

  moveToOther(paths, folder, separator) {
    return moveToOtherFolder(this.choices, this.left, paths, folder, separator);
  }

  merge(from, into) {
    mergeFolder(this.choices, this.duplicates, from, into);
  }

  /** Files marked Trash keep the folder alive. */
  leaveUnsorted(choice) {
    leaveUnsorted(this.choices, this.left, this.duplicates, choice);
  }

  movable(paths) {
    return paths.filter((path) => !markedTrash(this.choices, this.left, path)).length;
  }

  setEmptied(operations) {
    this.emptied = operations;
    this.tickEmptied(this.#emptiedWanted);
  }

  /** Operations that can't happen stay unticked. */
  tickEmptied(ticked) {
    this.#emptiedWanted = ticked;
    this.emptiedTicked = new Set(ticked ? this.emptied.filter((op) => !op.error).map((op) => op.id) : []);
  }
}
