// The steps after picking a Scan Root, in order: the scan, finding its Duplicate Groups, then the review
// screen with both, which reads the files for Suggested Folders from that same scan (ADR 0005).
// Kept apart from the screens, and given the commands it needs, so the order can be tested.
import { count, formatSize, plural } from "./format.js";

/**
 * The step flow over `api`, `{ scan, findDuplicates }` as in api.js, showing each step's screen with
 * `nav`. Each step rejects with the command's error, for the screen to show.
 */
export function stepFlow(api, nav) {
  return {
    /**
     * Scans `scanRoot` and finds its Duplicate Groups, calling `onProgress(line)` with a status line as
     * each goes, then shows the review screen.
     */
    async scanFolder(scanRoot, onProgress) {
      const result = await api.scan(scanRoot, (progress) => onProgress(describeScanProgress(scanRoot, progress)));
      const duplicates = await api.findDuplicates((progress) => onProgress(describeDuplicateProgress(progress)));
      nav.review(result, duplicates);
    },
  };
}

/** The scan's progress, for a status line: `{ found, folders, skipped, current }` while scanning `folder`. */
export function describeScanProgress(folder, { found, folders, skipped, current }) {
  const counts = [`${plural(found, "file")} found`, `${plural(folders, "folder")} read`, `${count(skipped)} protected skipped`];
  return `Scanning ${folder}…\n${counts.join(" · ")}\n${current}`;
}

/** Finding duplicates' progress, for a status line: `{ phase, finished, total, current }` as in api.js. */
export function describeDuplicateProgress({ phase, finished, total, current }) {
  const done =
    phase === "comparing"
      ? `Comparing files of the same size: ${count(finished)} of ${count(total)}…`
      : `Checking files that look alike: ${formatSize(finished)} of ${formatSize(total)}…`;
  return `${done}\n${current}`;
}
