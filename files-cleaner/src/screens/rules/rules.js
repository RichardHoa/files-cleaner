// Rules: the user's Rules (ADR 0013), in order, each read back in one line. A Rule is picked by clicking its
// row, then edited, turned off or on, moved or deleted from the toolbar, by double-clicking or pressing Return,
// Space, ⌥⌘↑/↓ or ⌘⌫; rows have no buttons. A file goes by the first enabled Rule it fits, so the order is
// the priority. The editor builds a Rule from ordinary controls, read back as it changes; a line of plain words
// above the list fills it with a draft, read by hand when they're a common phrasing or else by the describing
// model when the Grouping Models are in, saved only when the user saves it. Grouping by meaning needs the
// models too, so without them it isn't offered for a new Rule. Every
// change saves the whole list at once; a Rule that can't be saved says why and isn't saved. Rules apply at the
// next scan. A Rule that some file could share with an earlier one is marked, since that file goes by the
// earlier one (ADR 0014).
import { changeSetting, describeError, draftRule, groupingModelsStatus, loadConfig, ruleOverlaps } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { icon } from "../../shared/icons.js";
import { showStatus } from "../../shared/status.js";
import {
  describeGroupBy,
  describeMatch,
  describeRule,
  formatMeaningFolders,
  KINDS,
  MEGABYTE,
  NAME_HOW,
  newRule,
  parseExtensions,
  parseMeaningFolders,
  parseWords,
} from "./rule-text.js";

/**
 * `rule` in one row's words: what files it picks, then an arrow and where they go, set apart so the eye finds
 * the folder. Read aloud, it's the one line `describeRule` gives.
 */
const ruleText = (rule) =>
  el(
    "span",
    { className: "rules-text", ariaLabel: describeRule(rule) },
    el("span", { className: "rules-match" }, describeMatch(rule.match ?? {})),
    el("span", { className: "rules-arrow", ariaHidden: "true" }, "→"),
    el("span", { className: "rules-goes" }, describeGroupBy(rule.groupBy)),
  );

/** The group-by choices: one folder, a date grouping (`by`), by kind or by meaning. */
const GROUP_BY = [
  { value: "folder", label: "Into one folder" },
  { value: "buckets", label: "Today, Yesterday, Older…" },
  { value: "month", label: "By month" },
  { value: "year", label: "By year" },
  { value: "kind", label: "By kind" },
  { value: "meaning", label: "By meaning" },
];

/** `render(root, nav, from)`: `from` is "settings" when opened from Settings, so Back returns there. */
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

/**
 * The list of `initial` Rules in `body`, with its toolbar, or the editor in its place while one is edited. `withModels`
 * is whether the Grouping Models are in, to draft other phrasings and group by meaning.
 */
function rulesList(body, initial, withModels) {
  let rules = initial;
  let overlaps = [];
  let selected = null;
  const status = el("p", { className: "status" });
  const list = el("ul", { className: "rules-list", role: "listbox", ariaLabel: "Rules" });
  const add = el("button", { type: "button", className: "primary" }, icon("plus"), "Add Rule");
  const edit = el("button", { type: "button" }, "Edit");
  const toggle = el("button", { type: "button", title: "Space" }, "Turn Off");
  const up = el("button", { type: "button", title: "Move Up ⌥⌘↑", ariaLabel: "Move Up" }, icon("up"));
  const down = el("button", { type: "button", title: "Move Down ⌥⌘↓", ariaLabel: "Move Down" }, icon("down"));
  const remove = el("button", { type: "button", title: "⌘⌫" }, icon("trash"), "Delete");
  const toolbar = el("div", { className: "rules-toolbar" }, add, edit, toggle, up, down, remove);
  const phraseInput = el("input", {
    type: "text",
    placeholder: withModels ? "Put my files in folders by project…" : "Sort by month, .pdf into Papers…",
    ariaLabel: "Rule in plain words",
  });
  const draftButton = el("button", { type: "submit" }, "Draft");
  const phrase = el("form", { className: "rules-phrase" }, phraseInput, draftButton);
  const empty = el("p", { className: "empty" }, "No Rules yet.");

  /** Saves `next` as the whole list; resolves to whether it was saved, saying why not in `where`. */
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

  /** Marks the Rules some file could share with an earlier one; unmarked if that can't be worked out. */
  const findOverlaps = async () => {
    const shown = rules;
    const found = await ruleOverlaps(shown).catch(() => []);
    if (shown !== rules) return;
    overlaps = found;
    draw();
  };

  /** Opens the editor on the Rule at `index`, or on `drafted` (a new Rule when null) to add one. */
  const openEditor = (index, drafted = null) => {
    const editing = index === null ? (drafted ?? newRule()) : rules[index];
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

  /** Moves the selected Rule `by` places, so it wins over (or yields to) the Rules it passes. */
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
  edit.addEventListener("click", () => selected !== null && openEditor(selected));
  toggle.addEventListener("click", toggleSelected);
  up.addEventListener("click", () => moveSelected(-1));
  down.addEventListener("click", () => moveSelected(1));
  remove.addEventListener("click", deleteSelected);
  phrase.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!phraseInput.value.trim()) return;
    draftButton.disabled = true;
    // The model takes a few seconds to draft.
    draftButton.textContent = "Drafting…";
    try {
      const drafted = await draftRule(phraseInput.value);
      if (drafted) {
        phraseInput.value = "";
        openEditor(null, drafted);
      } else showStatus(status, "Couldn't understand that; try “by month” or “.pdf into Papers”, or add the Rule.", true);
    } catch (error) {
      showStatus(status, describeError(error), true);
    } finally {
      draftButton.disabled = false;
      draftButton.textContent = "Draft";
    }
  });

  function draw() {
    if (selected !== null && selected >= rules.length) selected = null;
    edit.disabled = toggle.disabled = remove.disabled = selected === null;
    up.disabled = selected === null || selected === 0;
    down.disabled = selected === null || selected === rules.length - 1;
    toggle.textContent = selected !== null && !rules[selected].enabled ? "Turn On" : "Turn Off";
    list.replaceChildren(
      ...rules.map((rule, index) => {
        // Its place in the order, since a file goes by the first Rule it fits.
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
          } else if (event.altKey && event.metaKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
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
    body.replaceChildren(phrase, toolbar, list, empty, status);
    draw();
  };
  show();
  findOverlaps();
}

/** A labelled control in the editor. */
const field = (label, ...controls) =>
  el("label", { className: "rules-field" }, el("span", { className: "rules-label" }, label), el("span", { className: "rules-controls" }, ...controls));

const select = (options, value) =>
  el("select", {}, ...options.map(([v, label]) => el("option", { value: v, selected: v === value }, label)));

/** Megabytes from `bytes`, or "" for none; a saved size reads back as it was. */
const megabytes = (bytes) => (bytes == null ? "" : String(bytes / MEGABYTE));

/**
 * The editor for `rule`, its match and group-by as controls and read back in one line below. Grouping by meaning
 * is offered only `withModels`, or when `rule` already does. `save(rule, where)` resolves to whether it was saved,
 * saying why not in `where`; `done(saved)` goes back to the list.
 */
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

  /** The Rule the controls show. */
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
