// A file goes by the first enabled Rule it fits (ADR 0013), so the order is the priority and
// overlaps are marked (ADR 0014).
import { changeSetting, describeError, groupingModelsStatus, loadConfig, ruleOverlaps } from "../../shared/api.js";
import { el, icon, showStatus } from "../../shared/dom.js";
import { formatSize } from "../../shared/format.js";
import { drawing } from "../../shared/introduction.js";
import { IS_MAC, withShortcut } from "../../shared/menus.js";

/** Split so the eye finds the folder; read aloud as one line. */
const ruleText = (rule) =>
  el(
    "span",
    { className: "rules-text", ariaLabel: describeRule(rule) },
    el("span", { className: "rules-match" }, describeMatch(rule.match ?? {})),
    el("span", { className: "rules-arrow", ariaHidden: "true" }, "→"),
    el("span", { className: "rules-goes" }, describeGroupBy(rule.groupBy)),
  );

const GROUP_BY = [
  { value: "folder", label: "Into one folder" },
  { value: "buckets", label: "Today, Yesterday, Older…" },
  { value: "month", label: "By month" },
  { value: "year", label: "By year" },
  { value: "kind", label: "By kind" },
  { value: "meaning", label: "By meaning" },
];

export function render(root, nav, from = "home") {
  const back = el("button", { type: "button" }, from === "settings" ? "Settings" : "Home");
  back.addEventListener("click", () => (from === "settings" ? nav.settings() : nav.home()));
  const body = el("div");
  root.append(el("div", { className: "actions" }, back), el("h1", {}, "Rules"), body);
  load(body);
}

async function load(body) {
  try {
    // Without the models' status, the Rules are still shown, as if they weren't in.
    const [{ config }, models] = await Promise.all([loadConfig(), groupingModelsStatus().catch(() => null)]);
    rulesList(body, config.rules, models?.state === "installed");
  } catch (error) {
    body.replaceChildren(el("p", { className: "status error" }, `The Rules couldn't be read: ${describeError(error)}`));
  }
}

/** Grouping by meaning is only offered `withModels`, as it needs them. */
function rulesList(body, initial, withModels) {
  let rules = initial;
  let overlaps = [];
  let selected = null;
  const status = el("p", { className: "status" });
  const list = el("ul", { className: "rules-list", role: "listbox", ariaLabel: "Rules" });
  const add = el("button", { type: "button", className: "primary" }, icon("plus"), "Add Rule");
  const edit = el("button", { type: "button", title: withShortcut("Edit", "edit") }, "Edit");
  const toggle = el("button", { type: "button" }, "Turn Off");
  const up = el("button", { type: "button", title: withShortcut("Move Up", "moveUp"), ariaLabel: "Move Up" }, icon("up"));
  const down = el("button", { type: "button", title: withShortcut("Move Down", "moveDown"), ariaLabel: "Move Down" }, icon("down"));
  const remove = el("button", { type: "button", title: withShortcut("Delete", "deleteRule") }, icon("trash"), "Delete");
  const toolbar = el("div", { className: "rules-toolbar" }, add, edit, toggle, up, down, remove);
  const start = el("button", { type: "button", className: "primary" }, "New Rule");
  const empty = el("div", { className: "rules-empty" }, drawing("rules"), el("p", {}, "Sort files your way at every scan"), start);

  const save = async (next, where = status) => {
    try {
      rules = (await changeSetting("rules", next)).config.rules;
      showStatus(status, "");
      findOverlaps();
      return true;
    } catch (error) {
      showStatus(where, describeError(error), true);
      return false;
    }
  };

  /** Unmarked if that can't be worked out: the Rules still work. */
  const findOverlaps = async () => {
    const shown = rules;
    const found = await ruleOverlaps(shown).catch(() => []);
    if (shown !== rules) return;
    overlaps = found;
    draw();
  };

  const openEditor = (index) => {
    const editing = index === null ? newRule() : rules[index];
    body.replaceChildren(
      editor(editing, withModels, {
        save: (rule, where) =>
          save(index === null ? [...rules, rule] : rules.map((r, i) => (i === index ? rule : r)), where),
        done: (saved) => {
          if (index !== null || saved) selected = index ?? rules.length - 1;
          show();
        },
      }),
    );
  };

  const deleteSelected = async () => {
    if (selected === null) return;
    if (await save(rules.filter((_, i) => i !== selected))) selected = null;
    draw();
  };

  const toggleSelected = async () => {
    if (selected === null) return;
    await save(rules.map((r, i) => (i === selected ? { ...r, enabled: !r.enabled } : r)));
    draw();
    list.children[selected]?.focus();
  };

  const moveSelected = async (by) => {
    if (selected === null) return;
    const to = selected + by;
    if (to < 0 || to >= rules.length) return;
    const next = [...rules];
    [next[selected], next[to]] = [next[to], next[selected]];
    if (await save(next)) selected = to;
    draw();
    list.children[selected]?.focus();
  };

  add.addEventListener("click", () => openEditor(null));
  start.addEventListener("click", () => openEditor(null));
  edit.addEventListener("click", () => selected !== null && openEditor(selected));
  toggle.addEventListener("click", toggleSelected);
  up.addEventListener("click", () => moveSelected(-1));
  down.addEventListener("click", () => moveSelected(1));
  remove.addEventListener("click", deleteSelected);

  function draw() {
    if (selected !== null && selected >= rules.length) selected = null;
    edit.disabled = toggle.disabled = remove.disabled = selected === null;
    up.disabled = selected === null || selected === 0;
    down.disabled = selected === null || selected === rules.length - 1;
    toggle.textContent = selected !== null && !rules[selected].enabled ? "Turn On" : "Turn Off";
    toggle.title = withShortcut(toggle.textContent, "toggle");
    list.replaceChildren(
      ...rules.map((rule, index) => {
        const classes = ["rules-row", index === selected ? "selected" : "", rule.enabled ? "" : "off"].filter(Boolean).join(" ");
        const row = el(
          "li",
          { className: classes, tabIndex: 0, role: "option", ariaSelected: String(index === selected) },
          el("span", { className: "rules-order muted", ariaHidden: "true" }, String(index + 1)),
          ruleText(rule),
          el(
            "span",
            { className: "rules-marks" },
            ...(overlaps[index] == null
              ? []
              : [
                  el(
                    "span",
                    { className: "rules-overlaps", title: `Files that fit both go by Rule ${overlaps[index] + 1}: “${describeRule(rules[overlaps[index]])}”` },
                    icon("alert"),
                    `Overlaps Rule ${overlaps[index] + 1}`,
                  ),
                ]),
            ...(rule.enabled ? [] : [el("span", { className: "pill" }, "Off")]),
          ),
        );
        if (rule.words) row.title = rule.words;
        row.addEventListener("click", () => {
          selected = index;
          draw();
          list.children[index]?.focus();
        });
        row.addEventListener("dblclick", () => openEditor(index));
        row.addEventListener("keydown", (event) => {
          if (event.key === "Enter") openEditor(index);
          else if (event.key === " ") {
            event.preventDefault();
            selected = index;
            toggleSelected();
          } else if (event.altKey && (IS_MAC ? event.metaKey : event.ctrlKey) && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
            event.preventDefault();
            selected = index;
            moveSelected(event.key === "ArrowUp" ? -1 : 1);
          } else if ((event.key === "Backspace" && event.metaKey) || event.key === "Delete") {
            selected = index;
            deleteSelected();
          }
        });
        return row;
      }),
    );
    list.hidden = rules.length === 0;
    empty.hidden = rules.length > 0;
  }

  const show = () => {
    body.replaceChildren(toolbar, list, empty, status);
    draw();
  };
  show();
  findOverlaps();
}

const field = (label, ...controls) =>
  el("label", { className: "rules-field" }, el("span", { className: "rules-label" }, label), el("span", { className: "rules-controls" }, ...controls));

const select = (options, value) =>
  el("select", {}, ...options.map(([v, label]) => el("option", { value: v, selected: v === value }, label)));

/** Not rounded, so a saved size reads back as it was. */
const megabytes = (bytes) => (bytes == null ? "" : String(bytes / MEGABYTE));

/** A Rule that already groups by meaning keeps that choice without the models. */
function editor(rule, withModels, { save, done }) {
  const match = rule.match ?? {};
  const date = match.date ?? null;
  const kind = select([["", "Any kind"], ...KINDS.map((k) => [k, k])], match.kind ?? "");
  const extensions = el("input", { type: "text", placeholder: "png, jpg", value: (match.extensions ?? []).join(", ") });
  const nameHow = select(NAME_HOW, match.name?.how ?? "contains");
  const name = el("input", { type: "text", value: (match.name?.words ?? []).join(", ") });
  const exceptNames = el("input", { type: "text", placeholder: "draft", title: "Leave out names containing any of these", value: (match.exceptNames ?? []).join(", ") });
  const minSize = el("input", { type: "number", min: 0, step: "any", className: "rules-number", value: megabytes(match.minSize) });
  const maxSize = el("input", { type: "number", min: 0, step: "any", className: "rules-number", value: megabytes(match.maxSize) });
  const dateField = select([["", "Any date"], ["changed", "Changed"], ["added", "Added"]], date?.field ?? "");
  const dateMode = select([["within", "in the last"], ["older", "over"]], date?.olderThanDays != null ? "older" : "within");
  const days = el("input", { type: "number", min: 0, className: "rules-number", value: String(date?.withinDays ?? date?.olderThanDays ?? 7) });
  const daysLabel = el("span", { className: "muted" });

  const groupBy = rule.groupBy;
  const groupChoice = select(
    GROUP_BY.filter(({ value }) => value !== "meaning" || withModels || groupBy.type === "meaning").map(({ value, label }) => [value, label]),
    groupBy.type === "date" ? groupBy.by : groupBy.type,
  );
  const folder = el("input", { type: "text", placeholder: "Screenshots/{month}", value: groupBy.name ?? "", title: "{year}, {month} and {day} are the file's date; / makes a folder inside" });
  const groupDate = select([["changed", "date changed"], ["added", "date added"]], groupBy.date ?? "changed");
  const folderField = field("Folder", folder);
  const meaningFolders = el("textarea", {
    rows: 3,
    placeholder: "Thesis: chapters and drafts\nTaxes",
    title: "One folder a line, what goes in it after a colon; none to propose folders from the files",
    value: formatMeaningFolders(groupBy.folders ?? []),
  });
  const meaningField = field("Folders", meaningFolders);
  const groupDateField = field("Using", groupDate);
  const readBack = el("p", { className: "rules-readback" });
  const status = el("p", { className: "status" });

  const read = () => {
    const m = {};
    if (kind.value) m.kind = kind.value;
    const exts = parseExtensions(extensions.value);
    if (exts.length > 0) m.extensions = exts;
    const words = parseWords(name.value);
    if (words.length > 0) m.name = { how: nameHow.value, words };
    const except = parseWords(exceptNames.value);
    if (except.length > 0) m.exceptNames = except;
    if (minSize.value !== "") m.minSize = Math.round(Number(minSize.value) * MEGABYTE);
    if (maxSize.value !== "") m.maxSize = Math.round(Number(maxSize.value) * MEGABYTE);
    if (dateField.value) {
      const n = Math.max(0, Math.round(Number(days.value) || 0));
      m.date = { field: dateField.value, ...(dateMode.value === "older" ? { olderThanDays: n } : { withinDays: n }) };
    }
    const choice = groupChoice.value;
    const g =
      choice === "folder"
        ? { type: "folder", name: folder.value.trim(), date: groupDate.value }
        : choice === "kind"
          ? { type: "kind" }
          : choice === "meaning"
            ? { type: "meaning", folders: parseMeaningFolders(meaningFolders.value) }
            : { type: "date", date: groupDate.value, by: choice };
    return { enabled: rule.enabled ?? true, ...(rule.words ? { words: rule.words } : {}), match: m, groupBy: g };
  };

  const update = () => {
    const off = !dateField.value;
    dateMode.disabled = days.disabled = off;
    const pattern = nameHow.value === "pattern";
    name.placeholder = pattern ? "Screenshot*" : "baraka, trip";
    name.title = pattern ? "* is any characters, ? is one" : "Any of these, separated by commas";
    daysLabel.textContent = dateMode.value === "older" ? "days ago" : "days";
    folderField.hidden = groupChoice.value !== "folder";
    meaningField.hidden = groupChoice.value !== "meaning";
    groupDateField.hidden = groupChoice.value === "kind" || groupChoice.value === "meaning";
    readBack.replaceChildren(ruleText(read()));
  };

  const saveButton = el("button", { type: "button", className: "primary" }, "Save");
  const cancel = el("button", { type: "button" }, "Cancel");
  saveButton.addEventListener("click", async () => {
    const next = read();
    if (next.groupBy.type === "folder" && !next.groupBy.name) {
      showStatus(status, "Name the folder.", true);
      folder.focus();
      return;
    }
    saveButton.disabled = true;
    if (await save(next, status)) done(true);
    else saveButton.disabled = false;
  });
  cancel.addEventListener("click", () => done(false));

  const form = el(
    "div",
    { className: "rules-editor" },
    el("h2", { className: "rules-group-title" }, "Files"),
    el(
      "div",
      { className: "rules-fields" },
      field("Kind", kind),
      field("Extensions", extensions),
      field("Name", nameHow, name),
      field("Except", exceptNames),
      field("Over", minSize, el("span", { className: "muted" }, "MB")),
      field("Under", maxSize, el("span", { className: "muted" }, "MB")),
      field("Date", dateField, dateMode, days, daysLabel),
    ),
    el("h2", { className: "rules-group-title" }, "Go"),
    el("div", { className: "rules-fields" }, field("Sort", groupChoice), folderField, meaningField, groupDateField),
    readBack,
    el("div", { className: "actions" }, cancel, saveButton),
    status,
  );
  form.addEventListener("input", update);
  form.addEventListener("change", update);
  update();
  return form;
}

// A Rule's shape is `Rule` in core, as saved in the config.

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
