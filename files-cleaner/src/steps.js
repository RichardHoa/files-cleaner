// Apart from the screens and given its commands, so the order can be tested.
import { count, formatSize, plural } from "./shared/format.js";

export function stepFlow(api, nav) {
  return {
    /** `cancelled()` is asked again at the end, as Cancel may come too late to stop the commands. */
    async scanFolder(scanRoot, onProgress, cancelled = () => false) {
      const result = await api.scan(scanRoot, (progress) => onProgress(describeScanProgress(scanRoot, progress)));
      const duplicates = await api.findDuplicates((progress) => onProgress(describeDuplicateProgress(progress)));
      if (!cancelled()) nav.review(result, duplicates);
    },
  };
}

export function describeScanProgress(folder, { found, folders, skipped, current }) {
  const counts = [`${plural(found, "file")} found`, `${plural(folders, "folder")} read`, `${count(skipped)} protected skipped`];
  return `Scanning ${folder}…\n${counts.join(" · ")}\n${current}`;
}

export function describeDuplicateProgress({ phase, finished, total, current }) {
  const done =
    phase === "comparing"
      ? `Comparing files of the same size: ${count(finished)} of ${count(total)}…`
      : `Checking files that look alike: ${formatSize(finished)} of ${formatSize(total)}…`;
  return `${done}\n${current}`;
}
