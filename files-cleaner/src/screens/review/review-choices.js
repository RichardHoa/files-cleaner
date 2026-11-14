// What the user chose on the review screen, and what that does to the files: for each Duplicate Group,
// which copies are kept and which are trashed; for each Suggested Folder, where it goes and what happens
// to each of its files. A folder's choice is "addTo" (into an existing folder, the subfolder it matched or one the user picked), "createHere"
// (a new folder in the Scan Root), "createIn" (a new folder inside `elsewhere`, a folder the user picked)
// or "trashAll"; each of its files is "inFolder" (goes wherever the folder's choice sends it), "trash" or
// "takeOut". The user can also add a folder, merge one into another, or remove one. The files left where they
// are stay there, unless the user trashes one or moves it to a folder, suggested or any other on disk.
// A folder's moves and its trashes, and the trashes among the files left where they are, can each be run on
// their own (ADR 0010): the files already run are kept in a Set `ran`, and a folder whose moves ran holds
// what that did in `moved`.
import { plural, splitPath } from "../../shared/format.js";

/** What the Plan calls the files trashed from those left where they are. */
const LEFT_NAME = "Left where they are";
/** What the Plan calls the files already moved or trashed by a part run on its own. */
const RAN_NAME = "Already run";

/** The files left where they are, and in `trashed` those of them the user trashes. A file that leaves is no longer trashed. */
export class LeftFiles extends Set {
  trashed = new Set();

  delete(path) {
    this.trashed.delete(path);
    return super.delete(path);
  }
}

/**
 * The starting choice for each Duplicate Group of `groups` (`{ size, copies }`, as `findDuplicates` gives
 * them): its first copy is kept and every other copy is trashed. Each copy is "keep" or "trash", by path.
 * `placed` is the copy holding the group's place among the Suggested Folders and the files left where they
 * are, `emptyPlace` where that was while no copy is kept, and `changedByHand` whether the user set one of
 * its copies themselves, which leaves it out of `setCopiesIn`.
 */
export function initialDuplicates(groups) {
  return groups.map((group) => ({
    group,
    copies: new Map(group.copies.map((copy, i) => [copy.path, i === 0 ? "keep" : "trash"])),
    placed: group.copies[0].path,
    emptyPlace: null,
    changedByHand: false,
  }));
}

/** The folder holding `copy`, from the Scan Root: "" for the Scan Root itself. */
export const copyFolder = (copy) => copy.relativePath.slice(0, -1).join("/");

/** The folders directly holding the copies of `duplicates`, each once, A–Z. */
export const foldersHoldingCopies = (duplicates) =>
  [...new Set(duplicates.flatMap((duplicate) => duplicate.group.copies.map(copyFolder)))].sort((a, b) => a.localeCompare(b));

/** How many copies of the Duplicate Group `duplicate` are trashed. */
export const trashedCopies = (duplicate) => [...duplicate.copies.values()].filter((fate) => fate === "trash").length;

/** The path of the Filed Copy of the Duplicate Group `duplicate`: its first Kept Copy, or null if no copy is kept. */
export const filedCopy = (duplicate) => [...duplicate.copies].find(([, fate]) => fate === "keep")?.[0] ?? null;

/** The Kept Copies of `duplicates` that aren't filed: they stay where they are, so they're listed with the files left there and can't be moved. */
export const otherKeptCopies = (duplicates) =>
  new Set(
    duplicates.flatMap((duplicate) => {
      const filed = filedCopy(duplicate);
      return [...duplicate.copies].filter(([path, fate]) => fate === "keep" && path !== filed).map(([path]) => path);
    }),
  );

/** The choice a Kept Copy takes over from the file whose place it takes: the same, except that a Trash clicked on that file was never about this copy. */
const handedOver = (file) => (file === "trash" ? "inFolder" : file);

/** Puts the file `to` wherever `from` is among `choices` and the Set `left`, with its choice handed over and in the same place in its folder. */
function replaceFile(choices, left, from, to) {
  if (from === to) return;
  for (const choice of choices) {
    if (!choice.files.has(from)) continue;
    choice.files = new Map([...choice.files].map(([path, file]) => (path === from ? [to, handedOver(file)] : [path, file])));
  }
  if (left.delete(from)) left.add(to);
}

/** Removes `path` from `choices` and the Set `left`, returning where it was: `{ choice, file }`, `choice` null for `left`, or null if it was in neither. */
function removeFile(choices, left, path) {
  const choice = choices.find((candidate) => candidate.files.has(path));
  if (choice) {
    const file = choice.files.get(path);
    choice.files.delete(path);
    return { choice, file };
  }
  return left.delete(path) ? { choice: null } : null;
}

/** Puts the Kept Copy `path` at `place`, as `removeFile` gave it. */
function putBack(left, path, { choice, file }) {
  if (choice) choice.files.set(path, handedOver(file));
  else left.add(path);
}

/**
 * Brings the copies of `duplicate` in line with its choices: the Filed Copy takes the group's place, needing
 * no reading as it's the same as the copy read, the other Kept Copies are listed in `left`, and a trashed
 * copy is nowhere. With no copy kept the group's place is remembered, for when one is kept again.
 */
function placeCopies(duplicate, choices, left) {
  const filed = filedCopy(duplicate);
  const { placed, emptyPlace } = duplicate;
  for (const path of duplicate.copies.keys()) {
    if (path !== placed) left.delete(path);
  }
  if (placed && filed) replaceFile(choices, left, placed, filed);
  else if (placed) duplicate.emptyPlace = removeFile(choices, left, placed);
  else if (filed && emptyPlace) putBack(left, filed, emptyPlace);
  duplicate.placed = filed;
  for (const path of otherKeptCopies([duplicate])) left.add(path);
}

/** The user sets the copy at `path` of the Duplicate Group `duplicate` to `fate`, "keep" or "trash": marks the group changed by hand and puts its copies where that leaves them. */
export function setCopy(duplicate, path, fate, choices, left) {
  duplicate.changedByHand = true;
  duplicate.copies.set(path, fate);
  placeCopies(duplicate, choices, left);
}

/** Sets every copy directly in `folder` to `fate`, "keep" or "trash", in the groups of `duplicates` not changed by hand, and puts their copies where that leaves them. */
export function setCopiesIn(duplicates, folder, fate, choices, left) {
  for (const duplicate of duplicates) {
    if (duplicate.changedByHand) continue;
    const held = duplicate.group.copies.filter((copy) => copyFolder(copy) === folder);
    if (held.length === 0) continue;
    for (const copy of held) duplicate.copies.set(copy.path, fate);
    placeCopies(duplicate, choices, left);
  }
}

/** Puts the copies of `duplicates` where the choices made while the files were read leave them; the copy read, the first, holds each group's place until then. */
export function placeDuplicates(choices, left, duplicates) {
  for (const duplicate of duplicates) {
    duplicate.placed = duplicate.group.copies[0].path;
    placeCopies(duplicate, choices, left);
  }
}

/** `duplicates` as `buildReviewPlan` takes them: each group's Filed Copy, or null, and the copies it trashes. */
export function planDuplicates(duplicates) {
  return duplicates
    .map((duplicate) => ({
      kept: filedCopy(duplicate),
      trash: [...duplicate.copies].filter(([, fate]) => fate === "trash").map(([path]) => path),
    }))
    .filter(({ trash }) => trash.length > 0);
}

/** The starting choice for each Suggested Folder of `folders`: into its matched subfolder if it has one, else a new folder in the Scan Root, with every file. */
export function initialChoices(folders) {
  return folders.map((folder) => ({
    folder,
    name: folder.name,
    choice: folder.existing ? "addTo" : "createHere",
    elsewhere: null,
    files: new Map(folder.files.map((path) => [path, "inFolder"])),
    moved: null,
  }));
}

/** Adds a Suggested Folder of the user's own to `choices`, with no files, going into the folder `existing` or else in the Scan Root, and returns it. */
export function addFolder(choices, name = "untitled folder", existing = null) {
  const [choice] = initialChoices([{ name, files: [], existing }]);
  choices.push(choice);
  return choice;
}

/** Takes the Suggested Folder `choice` off `choices`; a Duplicate Group that remembers its place there remembers `to` instead, the folder its files went to, or null for the files left where they are. */
function dropFolder(choices, duplicates, choice, to) {
  choices.splice(choices.indexOf(choice), 1);
  for (const { emptyPlace } of duplicates) {
    if (emptyPlace?.choice === choice) emptyPlace.choice = to;
  }
}

/** Merges the Suggested Folder `from` into `into`, which keeps its name and where it goes: each file comes along with the choice made on it. */
export function mergeFolder(choices, duplicates, from, into) {
  for (const [path, file] of from.files) into.files.set(path, file);
  dropFolder(choices, duplicates, from, into);
}

/** Removes the Suggested Folder `choice`, leaving every file of it where it is, in the Set `left`. */
export function removeFolder(choices, left, duplicates, choice) {
  for (const path of choice.files.keys()) left.add(path);
  dropFolder(choices, duplicates, choice, null);
}

/** What happens to `path` of the Suggested Folder `choice`: "move", "trash" or "stay", as for a file already directly in the folder it goes to. */
function fileFate(choice, path, separator) {
  const file = choice.files.get(path);
  if (file === "takeOut") return "stay";
  if (file === "trash" || choice.choice === "trashAll") return "trash";
  if (choice.choice === "addTo" && splitPath(path, separator).parent === choice.folder.existing) return "stay";
  return "move";
}

/**
 * `{ moved, trashed }`: how many files of all `choices` go into a folder, and how many are trashed, those
 * trashed from `left` and the copies `duplicates` trash included. Files in `ran`, already run, and the
 * moves of a folder whose moves ran, aren't counted.
 */
export function totals(choices, left, separator, duplicates = [], ran = new Set()) {
  const leftTrashed = [...left.trashed].filter((path) => !ran.has(path)).length;
  const sum = { moved: 0, trashed: leftTrashed + duplicates.reduce((n, duplicate) => n + trashedCopies(duplicate), 0) };
  for (const choice of choices) {
    for (const path of choice.files.keys()) {
      if (ran.has(path)) continue;
      const fate = fileFate(choice, path, separator);
      if (fate === "move" && !choice.moved) sum.moved += 1;
      if (fate === "trash") sum.trashed += 1;
    }
  }
  return sum;
}

/** What the review's `{ heading, button }` say about `totals`, and about `emptied`, the ticked Emptied Folders, when they're all that's left. */
export function describeChange({ moved, trashed }, emptied = 0) {
  if (moved > 0 && trashed > 0) return { heading: `${plural(moved, "file")} will move, ${trashed} will be trashed`, button: `Move ${moved}, trash ${trashed}` };
  if (moved > 0) return { heading: `${plural(moved, "file")} will move`, button: `Move ${plural(moved, "file")}` };
  if (trashed > 0) return { heading: `${plural(trashed, "file")} will be trashed`, button: `Trash ${plural(trashed, "file")}` };
  if (emptied > 0) return { heading: `${plural(emptied, "empty folder")} will be trashed`, button: `Trash ${plural(emptied, "empty folder")}` };
  return { heading: "Nothing will change", button: "Nothing to move" };
}

/**
 * What the sidebar says about the Suggested Folder `choice`: `going`, how many files go into it, and
 * `trashed`, whether every file it has goes to the System Trash, which is all of them under "Trash all".
 */
export function folderTally(choice) {
  const files = [...choice.files.values()];
  if (choice.choice === "trashAll") return { going: 0, trashed: files.length > 0 };
  return { going: files.filter((file) => file === "inFolder").length, trashed: files.length > 0 && files.every((file) => file === "trash") };
}

/** Whether the Suggested Folder `choice` lacks the name it needs: it needs one once it has files, and an empty name is never read as "trash". */
export const nameMissing = (choice) => choice.files.size > 0 && choice.name.trim() === "";

/** Moves `path` into the Suggested Folder `to`, in with the rest, or into the Set `left` of files left where they are when `to` is null. */
export function moveFile(choices, left, path, to) {
  for (const choice of choices) choice.files.delete(path);
  left.delete(path);
  if (to) to.files.set(path, "inFolder");
  else left.add(path);
}

/**
 * Moves `paths` into the Suggested Folder going into `folder`, a folder on disk the user picked, adding it if
 * there's none yet, and returns it. A file already directly in `folder` is left where it is instead, and
 * returns null when that's all of them.
 */
export function moveToOtherFolder(choices, left, paths, folder, separator) {
  const moving = paths.filter((path) => splitPath(path, separator).parent !== folder);
  for (const path of paths) {
    if (!moving.includes(path)) moveFile(choices, left, path, null);
  }
  if (moving.length === 0) return null;
  // A Disk Root has no name of its own, and a Suggested Folder needs one.
  const { name, parent } = splitPath(folder, separator);
  // A folder whose moves ran takes no more files.
  const to =
    choices.find((choice) => choice.choice === "addTo" && choice.folder.existing === folder && !choice.moved) ?? addFolder(choices, parent ? name : "Disk Root", folder);
  for (const path of moving) moveFile(choices, left, path, to);
  return to;
}

/** Where the Suggested Folder `choice` goes, as `buildReviewPlan` takes it. */
const planChoice = (choice) =>
  choice.choice === "addTo"
    ? { type: "addTo", folder: choice.folder.existing }
    : choice.choice === "createIn"
      ? { type: "createIn", folder: choice.elsewhere }
      : { type: choice.choice };

/** `paths` as a folder named `name` that trashes all of its own, as `buildReviewPlan` takes it, or nothing when there are none. */
const trashing = (name, paths) => (paths.length > 0 ? [{ name, choice: { type: "trashAll" }, files: paths.map((path) => [path, "inFolder"]) }] : []);

/** The files of the Suggested Folder `choice` that are trashed and not in `ran`, those already run. */
const trashedFiles = (choice, ran) =>
  [...choice.files].filter(([path, file]) => !ran.has(path) && (file === "trash" || (choice.choice === "trashAll" && file === "inFolder"))).map(([path]) => path);

/**
 * One part of the Suggested Folder `choice` as `buildReviewPlan` takes it, run on its own (ADR 0010): for
 * `part` "move", the files going into it; for "trash", those it trashes. Files in `ran`, already run, are left out.
 */
export function partChoices(choice, part, ran = new Set()) {
  if (part === "trash") return trashing(choice.name, trashedFiles(choice, ran));
  if (choice.choice === "trashAll") return [];
  const files = [...choice.files].filter(([path, file]) => file === "inFolder" && !ran.has(path));
  return files.length > 0 ? [{ name: choice.name, choice: planChoice(choice), files }] : [];
}

/**
 * What of `choices` hasn't run yet, as `buildReviewPlan` takes it: each folder with its files not in `ran`,
 * those already run, leaving out a folder with none, so no empty folder is made, and of a folder whose moves
 * ran, only its trashes; then the files of `left` being trashed, and last every file in `ran`, each as a folder
 * that trashes all of its own. What ran is there only so the folders it left empty are offered: its
 * Operations are never run again.
 */
export function planChoices(choices, left = new LeftFiles(), ran = new Set()) {
  const folders = choices.flatMap((choice) => {
    if (choice.moved) return partChoices(choice, "trash", ran);
    const files = [...choice.files].filter(([path]) => !ran.has(path));
    return files.length > 0 ? [{ name: choice.name, choice: planChoice(choice), files }] : [];
  });
  return [...folders, ...leftPartChoices(left, ran), ...trashing(RAN_NAME, [...ran])];
}

/** The files of `left` being trashed and not in `ran`, already run, as `buildReviewPlan` takes them: a folder that trashes all of its own. */
export const leftPartChoices = (left, ran = new Set()) => trashing(LEFT_NAME, [...left.trashed].filter((path) => !ran.has(path)));

/** `paths` by kind, A–Z with "Other" last, each kind's files newest first: `[[kind, paths], …]`. */
export function byKindNewestFirst(paths, kindOf, modifiedOf) {
  const kinds = new Map();
  for (const path of paths) {
    const kind = kindOf(path) ?? "Other";
    if (!kinds.has(kind)) kinds.set(kind, []);
    kinds.get(kind).push(path);
  }
  const rank = (kind) => (kind === "Other" ? 1 : 0);
  return [...kinds]
    .sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b))
    .map(([kind, files]) => [kind, files.sort((a, b) => (modifiedOf(b) ?? 0) - (modifiedOf(a) ?? 0))]);
}
