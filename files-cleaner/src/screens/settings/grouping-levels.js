// The grouping thresholds as the levels Settings shows instead of numbers. The config keeps storing
// thresholds: a level sends its threshold, and a stored value between levels is shown at the nearest one.
// The middle levels are today's default, 0.86.

/** How many Suggested Folders: `folderThreshold`, from looser (fewer, broader folders) to stricter. */
export const FOLDER_LEVELS = [
  { label: "A few broad folders", threshold: 0.8 },
  { label: "Fewer, broader folders", threshold: 0.83 },
  { label: "Some folders", threshold: 0.86 },
  { label: "More, narrower folders", threshold: 0.89 },
  { label: "Many specific folders", threshold: 0.92 },
];

/** How often a Suggested Folder goes into an existing folder: `existingFolderThreshold`, looser first. */
export const EXISTING_FOLDER_LEVELS = [
  { label: "Often", threshold: 0.82 },
  { label: "Sometimes", threshold: 0.86 },
  { label: "Rarely", threshold: 0.9 },
];

/** The index in `levels` of the level nearest the threshold `value`; the looser one on a tie. */
export function nearestLevel(levels, value) {
  let nearest = 0;
  for (const [index, level] of levels.entries()) {
    if (Math.abs(level.threshold - value) < Math.abs(levels[nearest].threshold - value)) nearest = index;
  }
  return nearest;
}

/** Whether `value` is a level's threshold, allowing for the config storing it as a 32-bit float. */
export const onLevel = (levels, value) => Math.abs(levels[nearestLevel(levels, value)].threshold - value) < 1e-4;
