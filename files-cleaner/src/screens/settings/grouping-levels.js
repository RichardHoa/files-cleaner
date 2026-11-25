// Levels instead of numbers, as a threshold means nothing to the user. The config keeps thresholds,
// so a value between levels shows at the nearest.

/** Looser means fewer, broader folders. */
export const FOLDER_LEVELS = [
  { label: "A few broad folders", threshold: 0.8 },
  { label: "Fewer, broader folders", threshold: 0.83 },
  { label: "Some folders", threshold: 0.86 },
  { label: "More, narrower folders", threshold: 0.89 },
  { label: "Many specific folders", threshold: 0.92 },
];

/** Looser first: files go into existing folders more often. */
export const EXISTING_FOLDER_LEVELS = [
  { label: "Often", threshold: 0.82 },
  { label: "Sometimes", threshold: 0.86 },
  { label: "Rarely", threshold: 0.9 },
];

/** The looser level on a tie. */
export function nearestLevel(levels, value) {
  let nearest = 0;
  for (const [index, level] of levels.entries()) {
    if (Math.abs(level.threshold - value) < Math.abs(levels[nearest].threshold - value)) nearest = index;
  }
  return nearest;
}

/** Approximate, as the config stores 32-bit floats. */
export const onLevel = (levels, value) => Math.abs(levels[nearestLevel(levels, value)].threshold - value) < 1e-4;
