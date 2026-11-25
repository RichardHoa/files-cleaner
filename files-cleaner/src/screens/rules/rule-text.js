// A Rule's shape is `Rule` in core, as saved in the config.
import { formatSize } from "../../shared/format.js";

/** Must match the grouping engine's names (`kinds.rs`). */
export const KINDS = [
  "Documents",
  "Spreadsheets",
  "Presentations",
  "Images",
  "Videos",
  "Audio",
  "Installers",
  "Archives",
  "Books",
  "Fonts",
  "Code",
  "Calendar and contacts",
  "Other",
];

/** Must match how `formatSize` counts. */
export const MEGABYTE = 1_000_000;

export const newRule = () => ({ enabled: true, match: {}, groupBy: { type: "folder", name: "", date: "changed" } });

export const parseExtensions = (text) =>
  text
    .split(/[\s,]+/)
    .map((e) => e.replace(/^\.+/, "").toLowerCase())
    .filter(Boolean);

/** Values must match `NameHow` in core. */
export const NAME_HOW = [
  ["contains", "contains"],
  ["startsWith", "starts with"],
  ["endsWith", "ends with"],
  ["is", "is"],
  ["pattern", "matches"],
];

/** Commas part the words, so a word can hold spaces. */
export const parseWords = (text) =>
  text
    .split(",")
    .map((w) => w.trim())
    .filter(Boolean);

/** One folder a line; a colon starts its description, as no folder name can hold one. */
export const parseMeaningFolders = (text) =>
  text
    .split("\n")
    .map((line) => {
      const at = line.indexOf(":");
      const name = (at < 0 ? line : line.slice(0, at)).trim();
      const description = at < 0 ? "" : line.slice(at + 1).trim();
      return description ? { name, description } : { name };
    })
    .filter((folder) => folder.name);

export const formatMeaningFolders = (folders) =>
  folders.map((f) => (f.description ? `${f.name}: ${f.description}` : f.name)).join("\n");

const anyOf = (words, joiner) => words.map((w) => `“${w}”`).join(` ${joiner} `);

export function describeMatch(match) {
  const extensions = (match.extensions ?? []).map((e) => `.${e}`).join(", ");
  let text = match.kind ?? "Files";
  if (extensions) text = match.kind ? `${text} (${extensions})` : `${extensions} files`;
  if (match.name?.words?.length) {
    const how = Object.fromEntries(NAME_HOW)[match.name.how ?? "contains"];
    text += ` whose name ${how} ${anyOf(match.name.words, "or")}`;
  }
  if (match.exceptNames?.length) text += ` except ${anyOf(match.exceptNames, "or")}`;
  const { minSize, maxSize, date } = match;
  if (minSize != null && maxSize != null) text += ` of ${formatSize(minSize)}–${formatSize(maxSize)}`;
  else if (minSize != null) text += ` over ${formatSize(minSize)}`;
  else if (maxSize != null) text += ` under ${formatSize(maxSize)}`;
  if (date) {
    const word = date.field === "added" ? "added" : "changed";
    if (date.withinDays === 0) text += ` ${word} today`;
    else if (date.withinDays != null) text += ` ${word} in the last ${date.withinDays} days`;
    if (date.olderThanDays != null) text += `${date.withinDays != null ? " and" : ""} ${word} over ${date.olderThanDays} days ago`;
  }
  return text;
}

export function describeGroupBy(groupBy) {
  if (groupBy.type === "folder") return `“${groupBy.name}”`;
  if (groupBy.type === "kind") return "by kind";
  if (groupBy.type === "meaning") {
    const names = (groupBy.folders ?? []).map((f) => f.name);
    return names.length ? `by meaning: ${anyOf(names, "or")}` : "by meaning";
  }
  const by = { buckets: "Today, Yesterday, Older…", month: "by month", year: "by year" }[groupBy.by];
  return groupBy.date === "added" ? `${by} added` : by;
}

export const describeRule = (rule) => `${describeMatch(rule.match ?? {})} → ${describeGroupBy(rule.groupBy)}`;
