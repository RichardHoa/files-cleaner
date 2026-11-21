// A Rule read back in one short line, and the small conversions the Rules screen's editor needs. A Rule is
// `{ enabled, words?, match, groupBy }` as saved in the config (see `Rule` in core): `match` is
// `{ kind?, extensions?, name?, exceptNames?, minSize?, maxSize?, date? }`, `name` `{ how, words }`,
// `date` `{ field, withinDays?, olderThanDays? }`,
// and `groupBy` `{ type: "folder", name, date }`, `{ type: "date", date, by }`, `{ type: "kind" }` or
// `{ type: "meaning", folders }`, each folder `{ name, description? }`.
import { formatSize } from "../../shared/format.js";

/** The file kinds as the grouping engine names them (`kinds.rs`). */
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

/** Bytes in the megabyte the editor counts sizes in, as `formatSize` counts them. */
export const MEGABYTE = 1_000_000;

/** A new Rule, before the user sets anything: every file into one folder. */
export const newRule = () => ({ enabled: true, match: {}, groupBy: { type: "folder", name: "", date: "changed" } });

/** "png, .JPG  gif" as `["png", "jpg", "gif"]`. */
export const parseExtensions = (text) =>
  text
    .split(/[\s,]+/)
    .map((e) => e.replace(/^\.+/, "").toLowerCase())
    .filter(Boolean);

/** How a name is held to a Rule's words (`NameHow` in core), with the editor's label for each. */
export const NAME_HOW = [
  ["contains", "contains"],
  ["startsWith", "starts with"],
  ["endsWith", "ends with"],
  ["is", "is"],
  ["pattern", "matches"],
];

/** "baraka, trip to" as `["baraka", "trip to"]`: commas part the words, so a word can hold spaces. */
export const parseWords = (text) =>
  text
    .split(",")
    .map((w) => w.trim())
    .filter(Boolean);

/** "Thesis: chapters and drafts\nTaxes" as `[{ name: "Thesis", description: "chapters and drafts" }, { name: "Taxes" }]`: one folder a line, what goes in it after a colon, which no folder name can hold. */
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

/** `folders` as `parseMeaningFolders` reads them. */
export const formatMeaningFolders = (folders) =>
  folders.map((f) => (f.description ? `${f.name}: ${f.description}` : f.name)).join("\n");

/** `words` as "“baraka” or “trip”". */
const anyOf = (words, joiner) => words.map((w) => `“${w}”`).join(` ${joiner} `);

/** What the files `match` picks have, as "Images whose name contains “baraka” over 1.0 MB". */
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

/** Where `groupBy` puts the files, as "“Screenshots/{month}”" or "by month changed". */
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

/** `rule` in one short line: "Images whose name matches “Screenshot*” → “Screenshots/{month}”". */
export const describeRule = (rule) => `${describeMatch(rule.match ?? {})} → ${describeGroupBy(rule.groupBy)}`;
