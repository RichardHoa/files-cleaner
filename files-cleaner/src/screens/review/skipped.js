// What the scan leaves out of the Suggested Folders, as chips under the review's scan summary: a short
// label each, with the detail in its tooltip.
import { count } from "../../shared/format.js";

/**
 * The chips for the scan `result`, each `{ label, title }`: subfolders and hidden files always, as every
 * scan skips them, and the others only when the scan met some.
 */
export function skippedChips({ skipped, noExtension, unreadable }) {
  return [
    {
      label: "Subfolders: duplicates only",
      title: "Only files directly in the folder are sorted into folders. Files deeper down are checked for duplicates.",
    },
    noExtension > 0 && {
      label: `No extension ${count(noExtension)}`,
      title: "Files without an extension, like the .pdf in report.pdf, are checked for duplicates but not sorted.",
    },
    skipped > 0 && {
      label: `Protected folders ${count(skipped)}`,
      title: "Folders the system and your apps need, and your Never-Touch Paths, are left alone.",
    },
    { label: "Hidden files & links", title: "Hidden files and symlinks are left alone." },
    unreadable.length > 0 && {
      label: `Unreadable ${count(unreadable.length)}`,
      title: unreadable.join("\n"),
    },
  ].filter(Boolean);
}
