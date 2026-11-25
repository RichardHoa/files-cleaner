// Pure rules on the user's choices, with no DOM, so they can be tested. A file marked Trash stays
// where it's listed, so moving never silently untrashes it.
import { count, splitPath } from "../../shared/format.js";

const LEFT_NAME = "Left where they are";

export class LeftFiles extends Set {
  trashed = new Set();

  delete(path) {
    this.trashed.delete(path);
    return super.delete(path);
  }
}

/**
 * `emptyPlace` remembers where a group was while no copy is kept, so keeping one again restores
 * it.
 */
export function initialDuplicates(groups) {
  return groups.map((group) => ({
    group,
    copies: new Map(group.copies.map((copy, i) => [copy.path, i === 0 ? "keep" : "trash"])),
    placed: group.copies[0].path,
    emptyPlace: null,
  }));
}

export const copyFolder = (copy) => copy.relativePath.slice(0, -1).join("/");

export const trashedCopies = (duplicate) => [...duplicate.copies.values()].filter((fate) => fate === "trash").length;

export const trashedCopiesIn = (duplicates) => duplicates.reduce((n, duplicate) => n + trashedCopies(duplicate), 0);

export const filedCopy = (duplicate) => [...duplicate.copies].find(([, fate]) => fate === "keep")?.[0] ?? null;

/** Not filed, so they can't be moved. */
export const otherKeptCopies = (duplicates) =>
  new Set(
    duplicates.flatMap((duplicate) => {
      const filed = filedCopy(duplicate);
      return [...duplicate.copies].filter(([path, fate]) => fate === "keep" && path !== filed).map(([path]) => path);
    }),
  );

/** A Trash clicked on the file it replaces was never about this copy. */
const handedOver = (file) => (file === "trash" ? "inFolder" : file);

function replaceFile(choices, left, from, to) {
  if (from === to) return;
  for (const choice of choices) {
    if (!choice.files.has(from)) continue;
    choice.files = new Map([...choice.files].map(([path, file]) => (path === from ? [to, handedOver(file)] : [path, file])));
  }
  if (left.delete(from)) left.add(to);
}

function removeFile(choices, left, path) {
  const choice = choices.find((candidate) => candidate.files.has(path));
  if (choice) {
    const file = choice.files.get(path);
    choice.files.delete(path);
    return { choice, file };
  }
  return left.delete(path) ? { choice: null } : null;
}

function putBack(left, path, { choice, file }) {
  if (choice) choice.files.set(path, handedOver(file));
  else left.add(path);
}

/** The Filed Copy takes the group's place without reading, as it is the same as the copy read. */
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

export function setCopy(duplicate, path, fate, choices, left) {
  duplicate.copies.set(path, fate);
  placeCopies(duplicate, choices, left);
}

/** Another copy is kept if none is, so one always survives. */
export function filedCopyTrashed(duplicate, choices, left) {
  const filed = filedCopy(duplicate);
  duplicate.copies.delete(filed);
  duplicate.group = { ...duplicate.group, copies: duplicate.group.copies.filter((copy) => copy.path !== filed) };
  if (!filedCopy(duplicate)) duplicate.copies.set(duplicate.group.copies[0].path, "keep");
  duplicate.placed = null;
  duplicate.emptyPlace = { choice: null };
  placeCopies(duplicate, choices, left);
}

/** Until now the copy read, the first, held each group's place. */
export function placeDuplicates(choices, left, duplicates) {
  for (const duplicate of duplicates) {
    duplicate.placed = duplicate.group.copies[0].path;
    placeCopies(duplicate, choices, left);
  }
}

export function planDuplicates(duplicates) {
  return duplicates
    .map((duplicate) => ({
      kept: filedCopy(duplicate),
      trash: [...duplicate.copies].filter(([, fate]) => fate === "trash").map(([path]) => path),
    }))
    .filter(({ trash }) => trash.length > 0);
}

export function initialChoices(folders) {
  return folders.map((folder) => ({
    folder,
    name: folder.name,
    choice: folder.existing ? "addTo" : "createHere",
    elsewhere: null,
    files: new Map(folder.files.map((path) => [path, "inFolder"])),
  }));
}

export function addFolder(choices, name = "untitled folder", existing = null) {
  const [choice] = initialChoices([{ name, files: [], existing }]);
  choices.push(choice);
  return choice;
}

/** A Duplicate Group may remember its place there, so that is redirected to `to`. */
function dropFolder(choices, duplicates, choice, to) {
  choices.splice(choices.indexOf(choice), 1);
  for (const { emptyPlace } of duplicates) {
    if (emptyPlace?.choice === choice) emptyPlace.choice = to;
  }
}

/** Each file keeps the choice made on it. */
export function mergeFolder(choices, duplicates, from, into) {
  for (const [path, file] of from.files) into.files.set(path, file);
  // Its files no longer share what the tool grouped them by.
  into.folder = { ...into.folder, basis: null };
  dropFolder(choices, duplicates, from, into);
}

/** Files marked Trash keep their folder until it is run. */
export function leaveUnsorted(choices, left, duplicates, choice) {
  for (const [path, file] of choice.files) {
    if (file === "trash") continue;
    choice.files.delete(path);
    left.add(path);
  }
  for (const { emptyPlace } of duplicates) {
    if (emptyPlace?.choice === choice && emptyPlace.file !== "trash") emptyPlace.choice = null;
  }
  if (choice.files.size === 0) dropFolder(choices, duplicates, choice, null);
}

function fileFate(choice, path, separator) {
  if (choice.files.get(path) === "trash") return "trash";
  if (choice.choice === "addTo" && splitPath(path, separator).parent === choice.folder.existing) return "stay";
  return "move";
}

export function totals(choices, left, separator, duplicates = []) {
  const sum = { moved: 0, trashed: left.trashed.size + trashedCopiesIn(duplicates) };
  for (const choice of choices) {
    for (const path of choice.files.keys()) {
      const fate = fileFate(choice, path, separator);
      if (fate === "move") sum.moved += 1;
      if (fate === "trash") sum.trashed += 1;
    }
  }
  return sum;
}

export function describeChange({ moved, trashed }, emptied = 0) {
  const parts = [...(moved > 0 ? [`Move ${count(moved)}`] : []), ...(trashed + emptied > 0 ? [`Trash ${count(trashed + emptied)}`] : [])];
  return parts.length > 0 ? parts.join(", ") : "Nothing to apply";
}

export function folderTally(choice) {
  const files = [...choice.files.values()];
  return { going: files.filter((file) => file === "inFolder").length, trashed: files.length > 0 && files.every((file) => file === "trash") };
}

/** An empty name must never be read as "trash". */
export const nameMissing = (choice) => choice.files.size > 0 && choice.name.trim() === "";

export const markedTrash = (choices, left, path) => Boolean(left.trashed?.has(path)) || choices.some((choice) => choice.files.get(path) === "trash");

/** A file marked Trash stays where it is, with its mark. */
export function moveFile(choices, left, path, to) {
  if (markedTrash(choices, left, path)) return;
  for (const choice of choices) choice.files.delete(path);
  left.delete(path);
  if (to) to.files.set(path, "inFolder");
  else left.add(path);
}

/** Null when nothing moved: files already in `folder` or marked Trash stay put. */
export function moveToOtherFolder(choices, left, paths, folder, separator) {
  const unmarked = paths.filter((path) => !markedTrash(choices, left, path));
  const moving = unmarked.filter((path) => splitPath(path, separator).parent !== folder);
  for (const path of unmarked) {
    if (!moving.includes(path)) moveFile(choices, left, path, null);
  }
  if (moving.length === 0) return null;
  // A Disk Root has no name of its own, and a Suggested Folder needs one.
  const { name, parent } = splitPath(folder, separator);
  const to = choices.find((choice) => choice.choice === "addTo" && choice.folder.existing === folder) ?? addFolder(choices, parent ? name : "Disk Root", folder);
  for (const path of moving) moveFile(choices, left, path, to);
  return to;
}

/** Nowhere when every file is trashed, so no empty folder is made. */
const planChoice = (choice) =>
  ![...choice.files.values()].includes("inFolder")
    ? { type: "trashAll" }
    : choice.choice === "addTo"
    ? { type: "addTo", folder: choice.folder.existing }
    : choice.choice === "createIn"
      ? { type: "createIn", folder: choice.elsewhere }
      : { type: choice.choice };

/** Folders with no files are left out, so no empty folder is made. */
export function planChoices(choices, left = new LeftFiles()) {
  const folders = choices.filter((choice) => choice.files.size > 0).map((choice) => ({ name: choice.name, choice: planChoice(choice), files: [...choice.files] }));
  const trashed = [...left.trashed];
  return [...folders, ...(trashed.length > 0 ? [{ name: LEFT_NAME, choice: { type: "trashAll" }, files: trashed.map((path) => [path, "inFolder"]) }] : [])];
}

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
