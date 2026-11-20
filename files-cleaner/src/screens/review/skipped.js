// What the scan leaves out of the Suggested Folders, as one line under the review's scan summary, with the
// detail opening under it on a click.
import { count } from "../../shared/format.js";

/** What every scan leaves out, said in the detail but never showing the line on its own. */
const ALWAYS = [
  "Only files directly in the folder are sorted into folders. Files deeper down are checked for duplicates.",
  "Hidden files and symlinks are left alone.",
];

/**
 * The line for the scan `result`, `{ label, detail }`, or null when it skipped nothing: no files without an
 * extension, no Protected Paths and nothing unreadable. The detail lists each with its count.
 */
export function skippedLine({ skipped, noExtension, unreadable }) {
  const lines = [
    noExtension > 0 && `Files without an extension: ${count(noExtension)}. They're checked for duplicates but not sorted.`,
    skipped > 0 && `Protected folders: ${count(skipped)}. Folders the system and your apps need, and your Never-Touch Paths, are left alone.`,
    unreadable.length > 0 && `Unreadable: ${count(unreadable.length)}\n${unreadable.join("\n")}`,
  ].filter(Boolean);
  return lines.length > 0 ? { label: "Some files skipped", detail: [...lines, "", ...ALWAYS].join("\n") } : null;
}
