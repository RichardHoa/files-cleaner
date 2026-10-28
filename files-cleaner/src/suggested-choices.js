// What the user chose for each Suggested Folder, and what that does to its files. A folder's choice
// is "addTo" (into the existing subfolder it matched), "createHere" (a new folder in the Scan Root),
// "createIn" (a new folder inside `elsewhere`, a folder the user picked), "trashAll" or "skip"; each
// of its files is "inFolder" (goes wherever the folder's choice sends it), "trash" or "takeOut".

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

/** `{ moved, trashed }`: how many files of all `choices` go into a folder, and how many are trashed. */
export function totals(choices) {
  const sum = { moved: 0, trashed: 0 };
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

/** `choices` as `buildSuggestedFoldersPlan` takes them, leaving out those with no files, so no empty folder is made. */
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
