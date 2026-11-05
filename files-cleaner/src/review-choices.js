// What the user chose on the review screen, and what that does to the files: for each Duplicate Group,
// which copies are kept and which are trashed; for each Suggested Folder, where it goes and what happens
// to each of its files. A folder's choice is "addTo" (into the existing subfolder it matched), "createHere"
// (a new folder in the Scan Root), "createIn" (a new folder inside `elsewhere`, a folder the user picked),
// "trashAll" or "skip"; each of its files is "inFolder" (goes wherever the folder's choice sends it),
// "trash" or "takeOut".

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
  }));
}

/** What happens to `path` of the Suggested Folder `choice`: "move", "trash" or "stay". */
function fileFate(choice, path) {
  const file = choice.files.get(path);
  if (choice.choice === "skip" || file === "takeOut") return "stay";
  if (file === "trash" || choice.choice === "trashAll") return "trash";
  return "move";
}

/**
 * `{ moved, trashed }`: how many files of all `choices` go into a folder, and how many are trashed, the
 * copies `duplicates` trash included.
 */
export function totals(choices, duplicates = []) {
  const sum = { moved: 0, trashed: duplicates.reduce((n, duplicate) => n + trashedCopies(duplicate), 0) };
  for (const choice of choices) {
    for (const path of choice.files.keys()) {
      const fate = fileFate(choice, path);
      if (fate === "move") sum.moved += 1;
      if (fate === "trash") sum.trashed += 1;
    }
  }
  return sum;
}

/** Whether the Suggested Folder `choice` lacks the name it needs: any choice but skip needs one once it has files, and an empty name is never read as "trash". */
export const nameMissing = (choice) => choice.choice !== "skip" && choice.files.size > 0 && choice.name.trim() === "";

/** Moves `path` into the Suggested Folder `to`, in with the rest, or into the Set `left` of files left where they are when `to` is null. */
export function moveFile(choices, left, path, to) {
  for (const choice of choices) choice.files.delete(path);
  left.delete(path);
  if (to) to.files.set(path, "inFolder");
  else left.add(path);
}

/** `choices` as `buildReviewPlan` takes them, leaving out those with no files, so no empty folder is made. */
export function planChoices(choices) {
  return choices.filter((choice) => choice.files.size > 0).map((choice) => ({
    name: choice.name,
    choice:
      choice.choice === "addTo"
        ? { type: "addTo", folder: choice.folder.existing }
        : choice.choice === "createIn"
          ? { type: "createIn", folder: choice.elsewhere }
          : { type: choice.choice },
    files: [...choice.files],
  }));
}

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
