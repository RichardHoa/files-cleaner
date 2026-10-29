// What the user chose on the review screen, and what that does to the files: for each Duplicate Group,
// which copy stays and which are trashed; for each Suggested Folder, where it goes and what happens to
// each of its files. A folder's choice is "addTo" (into the existing subfolder it matched), "createHere"
// (a new folder in the Scan Root), "createIn" (a new folder inside `elsewhere`, a folder the user
// picked), "trashAll" or "skip"; each of its files is "inFolder" (goes wherever the folder's choice
// sends it), "trash" or "takeOut".

/**
 * The starting choice for each Duplicate Group of `groups` (`{ size, copies }`, as `findDuplicates` gives
 * them): its first copy, the default Keeper, stays, and every other copy is trashed. Each copy is
 * "keeper" or "trash", by path.
 */
export function initialDuplicates(groups) {
  return groups.map((group) => ({
    group,
    copies: new Map(group.copies.map((copy, i) => [copy.path, i === 0 ? "keeper" : "trash"])),
  }));
}

/** How many copies of the Duplicate Group `duplicate` are trashed. */
export const trashedCopies = (duplicate) => [...duplicate.copies.values()].filter((fate) => fate === "trash").length;

/** `duplicates` as `buildReviewPlan` takes them: each group's Keeper and the copies it trashes. */
export function planDuplicates(duplicates) {
  return duplicates
    .map(({ copies }) => {
      const paths = (fate) => [...copies].filter(([, f]) => f === fate).map(([path]) => path);
      return { keeper: paths("keeper")[0], trash: paths("trash") };
    })
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
