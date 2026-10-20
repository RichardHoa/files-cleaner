// The steps after picking a Scan Root, in order: the Scan report, the duplicate step (its own Plan
// and Session, or skipped), then a fresh scan of the same Scan Root and the Suggested Folders step.
// Kept apart from the screens, and given the commands it needs, so the order can be tested.
import { count, plural } from "./format.js";

/**
 * The step flow over `api`, `{ buildPlan, scan }` as in api.js, showing each step's screen with
 * `nav`. Each step rejects with the command's error, for the screen to show.
 */
export function stepFlow(api, nav) {
  return {
    /** From the Scan report `result`: plans the duplicate step and shows that Plan, which may be empty. */
    async findDuplicates(result) {
      nav.plan(result, await api.buildPlan());
    },

    /**
     * Once the duplicate step was run, skipped, or had no duplicates: scans `scanRoot` afresh, calling
     * `onProgress` as it goes, and shows the Suggested Folders step with that scan. Grouping never gets
     * the scan from before the duplicate step, as its copies may be gone.
     */
    async continueAfterDuplicates(scanRoot, onProgress) {
      nav.suggestedFolders(await api.scan(scanRoot, onProgress));
    },
  };
}

/** What a Session screen opened right after the duplicate Session of `scanRoot` is given as `from`. */
export const fromDuplicateStep = (scanRoot) => ({ step: "duplicates", scanRoot });

/** The scan's progress, for a status line: `{ found, folders, skipped, current }` while scanning `folder`. */
export function describeScanProgress(folder, { found, folders, skipped, current }) {
  const counts = [`${plural(found, "file")} found`, `${plural(folders, "folder")} read`, `${count(skipped)} protected skipped`];
  return `Scanning ${folder}…\n${counts.join(" · ")}\n${current}`;
}
