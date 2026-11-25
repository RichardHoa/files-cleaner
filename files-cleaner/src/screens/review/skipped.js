// One quiet line: what was left out matters only to some.
import { count } from "../../shared/format.js";

/** True of every scan, so not worth showing the line for. */
const ALWAYS = [
  "Only files directly in the folder are sorted into folders. Files deeper down are checked for duplicates.",
  "Hidden files and symlinks are left alone.",
];

export function skippedLine({ skipped, noExtension, unreadable }) {
  const lines = [
    noExtension > 0 && `Files without an extension: ${count(noExtension)}. They're checked for duplicates but not sorted.`,
    skipped > 0 && `Protected folders: ${count(skipped)}. Folders the system and your apps need, and your Never-Touch Paths, are left alone.`,
    unreadable.length > 0 && `Unreadable: ${count(unreadable.length)}\n${unreadable.join("\n")}`,
  ].filter(Boolean);
  return lines.length > 0 ? { label: "Some files skipped", detail: [...lines, "", ...ALWAYS].join("\n") } : null;
}
