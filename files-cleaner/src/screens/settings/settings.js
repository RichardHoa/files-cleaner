// Settings: the Never-Touch Paths, the Default Copy Rule, how files are grouped,
// notifications, the Grouping Models, deleting all Sessions, and help. The headings say these in plain
// words ("Folders to leave alone"); the glossary and the code keep the exact terms.
// This is the only way settings change: the config file behind them is never edited by hand. Each change
// is saved as soon as it's made; an invalid value says why beside it and isn't saved. Grouping changes
// apply the next time files are grouped.
import {
  changeSetting,
  clearGroupingCards,
  deleteAllSessions,
  describeError,
  downloadDescribingModel,
  listSessions,
  loadConfig,
  openWebPage,
  pathSeparator,
  pauseDescribingModel,
  pickFolder,
  removeDescribingModel,
  removeGroupingModels,
  resetSettings,
  revealInFileManager,
  useDescribingModel,
} from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { formatSize, plural, splitPath, trashPlace } from "../../shared/format.js";
import { showAllGuidesAgain } from "../../shared/onboarding/guide-card.js";
import { showIntroduction } from "../../shared/onboarding/introduction.js";
import { groupingModelsControls, modelsButton, watchGroupingModels } from "../../shared/grouping-models/models-panel.js";
import { warningBlock } from "../../shared/outcomes.js";
import { progressBar } from "../../shared/progress-bar.js";
import { showStatus } from "../../shared/status.js";
import { EXISTING_FOLDER_LEVELS, FOLDER_LEVELS, nearestLevel, onLevel } from "./grouping-levels.js";

/** What this OS calls its file manager, for "Show in Finder". */
const FILE_MANAGER = pathSeparator === "\\" ? "Explorer" : /Mac/.test(navigator.userAgent) ? "Finder" : "File Manager";

/** The Default Copy Rule's choices. */
const COPY_RULES = [
  { value: "newest", label: "Newest", detail: "the copy modified last" },
  { value: "oldest", label: "Oldest", detail: "the copy modified longest ago" },
  { value: "shortestPath", label: "Shortest path", detail: "the copy with the shortest folder and file name" },
];


export function render(root, nav) {
  const home = el("button", { type: "button" }, "Home");
  home.addEventListener("click", () => nav.home());
  const configSections = el("div");

  root.append(
    el("div", { className: "actions" }, home),
    el("h1", {}, "Settings"),
    configSections,
    readAgainSection(),
    modelsSection(),
    sessionsSection(),
    helpSection(),
    iconCredit(),
  );
  showConfigSections(configSections);
}

/** Replays the introduction, or brings back the guide cards the user dismissed. */
function helpSection() {
  const status = el("p", { className: "status" });
  const replay = el("button", { type: "button" }, "Replay introduction");
  replay.addEventListener("click", () => showIntroduction());
  const guides = el("button", { type: "button" }, "Show all guides again");
  guides.addEventListener("click", async () => {
    await showAllGuidesAgain();
    showStatus(status, "Each screen shows its guide again the next time you open it.");
  });
  return section("Help", el("div", { className: "actions" }, replay, guides), status);
}

/** Credits the app icon's author, linking to their page in the default browser. */
function iconCredit() {
  const author = el("button", { type: "button", className: "link" }, "juicy_fish");
  author.addEventListener("click", () => openWebPage("https://www.flaticon.com/authors/juicy-fish"));
  return el("p", { className: "note" }, "App icon by ", author, " on Flaticon.");
}

/** The sections kept in the config, or why it can't be read, with "Reset settings" to start again from the defaults. */
async function showConfigSections(node) {
  let config;
  try {
    ({ config } = await loadConfig());
  } catch (error) {
    const reset = el("button", { type: "button", className: "primary" }, "Reset settings");
    const status = el("p", { className: "status" });
    reset.addEventListener("click", async () => {
      reset.disabled = true;
      try {
        await resetSettings();
        showConfigSections(node);
      } catch (e) {
        showStatus(status, describeError(e), true);
        reset.disabled = false;
      }
    });
    node.replaceChildren(
      el("p", { className: "status error" }, `The settings couldn't be read: ${describeError(error)}`),
      el("p", { className: "note" }, "Reset settings puts every setting back to its default, including the Never-Touch Paths."),
      el("div", { className: "actions" }, reset),
      status,
    );
    return;
  }
  node.replaceChildren(
    neverTouchSection(config.neverTouch),
    duplicatesSection(config.defaultCopyRule),
    groupingSection(config.grouping),
    notificationsSection(config.notifications),
  );
}

/** `title`'s section, holding `children`. */
const section = (title, ...children) => el("section", { className: "settings-section" }, el("h2", {}, title), ...children);

/** One radio button of the group `name`, with `label` and a muted `detail`. */
const choice = (name, label, detail, props) =>
  el(
    "label",
    { className: "settings-choice" },
    el("input", { type: "radio", name, ...props }),
    el("span", {}, label, el("span", { className: "muted" }, ` · ${detail}`)),
  );

/** The Default Copy Rule: which copy of each Duplicate Group starts out kept. A failure says why and puts the choice back. */
function duplicatesSection(initial) {
  const status = el("p", { className: "status" });
  const choices = el(
    "fieldset",
    { className: "settings-choices" },
    el("legend", { className: "note" }, "Every other copy starts out trashed."),
    ...COPY_RULES.map(({ value, label, detail }) => choice("default-copy-rule", label, detail, { value, checked: value === initial })),
  );
  let rule = initial;
  choices.addEventListener("change", async ({ target }) => {
    try {
      ({ defaultCopyRule: rule } = (await changeSetting("defaultCopyRule", target.value)).config);
      showStatus(status, "");
    } catch (error) {
      choices.querySelector(`input[value="${rule}"]`).checked = true;
      showStatus(status, describeError(error), true);
    }
  });
  return section("When files are identical, keep…", choices, el("p", { className: "note" }, "Changes apply the next time a folder is scanned."), status);
}

function neverTouchSection(initial) {
  const list = el("ul", { className: "settings-list" });
  const status = el("p", { className: "status" });
  const add = el("button", { type: "button" }, "Add…");
  let paths = initial;

  const save = async (next) => {
    try {
      ({ neverTouch: paths } = (await changeSetting("neverTouch", next)).config);
      showStatus(status, "");
      draw();
    } catch (error) {
      showStatus(status, describeError(error), true);
    }
  };
  add.addEventListener("click", async () => {
    try {
      const folder = await pickFolder("Pick a folder to never touch");
      if (folder && !paths.includes(folder)) save([...paths, folder]);
    } catch (error) {
      showStatus(status, describeError(error), true);
    }
  });

  function draw() {
    list.replaceChildren(
      ...paths.map((path) => {
        const remove = el("button", { type: "button", className: "link" }, "Remove");
        remove.addEventListener("click", () => save(paths.filter((p) => p !== path)));
        return el("li", {}, el("span", { className: "path" }, path), remove);
      }),
    );
    list.hidden = paths.length === 0;
  }
  draw();
  return section(
    "Folders to leave alone",
    el("p", { className: "note" }, "Folders the app leaves alone, everything inside them included. System and app folders are left alone already."),
    list,
    el("div", { className: "actions" }, add),
    status,
  );
}

function groupingSection(grouping) {
  return section(
    "Folder suggestions",
    el("p", { className: "note" }, "Changes apply the next time files are grouped."),
    folderLevelSetting(grouping.folderThreshold),
    existingFolderSetting(grouping.existingFolderThreshold),
    smallestFolderSetting(grouping.smallestFolder),
  );
}

/** Saves `value` (null for the default) as the grouping setting `key`; resolves to the value saved, or shows why not in `error` and resolves to undefined. */
async function saveGrouping(key, value, error) {
  try {
    const saved = (await changeSetting(key, value)).config.grouping[key];
    showStatus(error, "");
    return saved;
  } catch (e) {
    showStatus(error, describeError(e), true);
    return undefined;
  }
}

/**
 * The grouping threshold `key` as one of `levels`, shown by `show(index)`; returns `save(threshold)`, null
 * for the default, which shows the level saved, or puts the last one back if saving fails. A stored value
 * between levels is snapped to the nearest one and saved, without a word.
 */
function levelSetting(key, levels, initial, show, error) {
  let level = nearestLevel(levels, initial);
  const save = async (threshold) => {
    const saved = await saveGrouping(key, threshold, error);
    if (saved !== undefined) level = nearestLevel(levels, saved);
    show(level);
  };
  show(level);
  if (!onLevel(levels, initial)) save(levels[level].threshold);
  return save;
}

/** How many Suggested Folders: five levels on a slider, from a few broad folders to many specific ones. */
function folderLevelSetting(initial) {
  const slider = el("input", { type: "range", min: 0, max: FOLDER_LEVELS.length - 1, step: 1, ariaLabel: "How many folders" });
  const current = el("span", { className: "muted" });
  const reset = el("button", { type: "button", className: "link" }, "Reset to default");
  const error = el("p", { className: "status" });
  const show = (index) => {
    slider.value = index;
    current.textContent = FOLDER_LEVELS[index].label;
    slider.ariaValueText = FOLDER_LEVELS[index].label;
  };
  const save = levelSetting("folderThreshold", FOLDER_LEVELS, initial, show, error);
  slider.addEventListener("input", () => show(Number(slider.value)));
  slider.addEventListener("change", () => save(FOLDER_LEVELS[Number(slider.value)].threshold));
  reset.addEventListener("click", () => save(null));

  const [fewest, most] = [FOLDER_LEVELS[0], FOLDER_LEVELS.at(-1)];
  return el(
    "div",
    { className: "settings-setting" },
    el("div", { className: "settings-setting-head" }, el("strong", {}, "How many folders"), current, reset),
    el(
      "div",
      { className: "settings-slider" },
      el("span", { className: "muted" }, fewest.label),
      slider,
      el("span", { className: "muted" }, most.label),
    ),
    error,
  );
}

/** How often a Suggested Folder goes into a folder already in the scanned folder: Often, Sometimes or Rarely. */
function existingFolderSetting(initial) {
  const reset = el("button", { type: "button", className: "link" }, "Reset to default");
  const error = el("p", { className: "status" });
  const choices = el(
    "fieldset",
    { className: "settings-choices" },
    ...EXISTING_FOLDER_LEVELS.map(({ label }, index) =>
      el("label", { className: "settings-choice" }, el("input", { type: "radio", name: "existing-folders", value: index }), el("span", {}, label)),
    ),
  );
  const show = (index) => (choices.querySelector(`input[value="${index}"]`).checked = true);
  const save = levelSetting("existingFolderThreshold", EXISTING_FOLDER_LEVELS, initial, show, error);
  choices.addEventListener("change", ({ target }) => save(EXISTING_FOLDER_LEVELS[Number(target.value)].threshold));
  reset.addEventListener("click", () => save(null));

  return el(
    "div",
    { className: "settings-setting" },
    el("div", { className: "settings-setting-head" }, el("strong", {}, "Use your existing folders"), reset),
    el("p", { className: "note" }, "How often a suggested folder goes into a folder already in the scanned folder, rather than a new one."),
    choices,
    error,
  );
}

/** The fewest files a Suggested Folder holds, 2–10, as a sentence with a stepper. */
function smallestFolderSetting(initial) {
  const number = el("input", { type: "number", min: 2, max: 10, step: 1, value: initial, className: "settings-number", ariaLabel: "Fewest files in a folder" });
  const reset = el("button", { type: "button", className: "link" }, "Reset to default");
  const error = el("p", { className: "status" });
  const save = async (value) => {
    const saved = await saveGrouping("smallestFolder", value, error);
    if (saved !== undefined) {
      number.value = saved;
      number.ariaInvalid = "false";
    }
  };
  number.addEventListener("change", () => {
    const value = number.valueAsNumber;
    if (!(Number.isInteger(value) && value >= 2 && value <= 10)) {
      number.ariaInvalid = "true";
      showStatus(error, "Enter a whole number from 2 to 10.", true);
      return;
    }
    save(value);
  });
  reset.addEventListener("click", () => save(null));
  return el(
    "div",
    { className: "settings-setting" },
    el("div", { className: "settings-setting-head" }, el("label", {}, "Only suggest a folder for at least ", number, " files"), reset),
    error,
  );
}

/** Whether to notify the user when the folders are ready, or reading failed, while the app is in the background. */
function notificationsSection(initial) {
  const box = el("input", { type: "checkbox", checked: initial === true });
  const status = el("p", { className: "status" });
  box.addEventListener("change", async () => {
    try {
      box.checked = (await changeSetting("notifications", box.checked)).config.notifications === true;
      showStatus(status, "");
    } catch (error) {
      box.checked = !box.checked;
      showStatus(status, describeError(error), true);
    }
  });
  return section(
    "Notifications",
    el("label", { className: "settings-choice" }, box, el("span", {}, "Notify me when the Suggested Folders are ready")),
    el("p", { className: "note" }, "Only while Files Cleaner is in the background, so you can switch to other apps while files are read."),
    status,
  );
}

/** "Read every file again": forgets what the Grouping Models read, so the next grouping reads every file afresh. */
function readAgainSection() {
  const button = el("button", { type: "button" }, "Read every file again");
  const status = el("p", { className: "status" });
  button.addEventListener("click", async () => {
    button.disabled = true;
    try {
      await clearGroupingCards();
      showStatus(status, "Done. The next grouping reads every file again.");
    } catch (error) {
      showStatus(status, describeError(error), true);
      button.disabled = false;
    }
  });
  return el(
    "div",
    { className: "settings-setting" },
    el("div", { className: "actions" }, button),
    el("p", { className: "note" }, "Forgets what was read from your files before, so the next grouping reads them all afresh. It takes as long as the first time."),
    status,
  );
}

/** The Grouping Models: their status and controls, each model's name, size on disk and folder, and "Advanced". */
function modelsSection() {
  const body = el("div");
  const error = el("p", { className: "status error" });
  const advanced = describingModelSetting();
  const node = section("Reading inside files", body, error, advanced.node);
  const bar = progressBar();
  watchGroupingModels(node, error, (status) => {
    advanced.show(status);
    const onDisk = status.models.filter((model) => model.path);
    const total = onDisk.reduce((sum, model) => sum + model.size, 0);
    const installed = status.state === "installed";
    body.replaceChildren(
      ...(installed ? [el("p", {}, "Installed. Files are grouped by what's inside them.")] : groupingModelsControls(status, error, bar)),
      ...(onDisk.length ? [modelList(onDisk, error), el("p", { className: "muted" }, `Total: ${formatSize(total)}`)] : []),
      installed ? removeModels(status.total, error) : "",
    );
    // A failed download says why in the orange warning, in place of its text.
    showStatus(error, "");
  });
  return node;
}

/**
 * "Advanced": a box for a Hugging Face link to one `.gguf` file, to describe files with that model instead of
 * the default (ADR 0009), and the picked model's download, check or switch. Built once and fed each status,
 * so what's typed in the box stays; shown only once the Grouping Models are installed.
 */
function describingModelSetting() {
  const link = el("input", {
    type: "url",
    className: "settings-link",
    placeholder: "https://huggingface.co/…/blob/main/model.gguf",
    ariaLabel: "Hugging Face link to one .gguf file",
  });
  const download = el("button", { type: "button" }, "Download");
  const current = el("div");
  const error = el("p", { className: "status error" });
  const bar = progressBar();
  const node = el(
    "details",
    { className: "settings-setting" },
    el("summary", {}, "Advanced"),
    el(
      "p",
      { className: "note" },
      "Describe your files with another model from Hugging Face: paste the link to one .gguf file of a chat model. " +
        "It's tried on a sample file before it's used. The model that compares the descriptions stays the same.",
    ),
    current,
    el("div", { className: "actions" }, link, download),
    error,
  );

  download.addEventListener("click", async () => {
    if (!link.value.trim()) return;
    download.disabled = true;
    showStatus(error, "");
    try {
      await downloadDescribingModel(link.value);
      link.value = "";
    } catch (e) {
      showStatus(error, describeError(e), true);
      download.disabled = false;
    }
  });
  link.addEventListener("keydown", (event) => event.key === "Enter" && download.click());

  return {
    node,
    show(status) {
      node.hidden = status.state !== "installed";
      const running = status.custom != null && status.custom.state !== "installed";
      link.disabled = download.disabled = running;
      current.replaceChildren(...describingModelControls(status, error, bar));
    },
  };
}

/**
 * What describes files now, or the picked model's download with its progress in `bar`, or its check. A
 * download that failed, or a model refused, says why in the orange warning; a failing button says why in `error`.
 */
function describingModelControls({ custom, customError, models }, error, bar) {
  const button = (label, command, primary = false) => modelsButton(label, command, error, primary);
  const warning = customError ? [warningBlock(customError)] : [];
  const byDefault = `Your files are described by ${models[0].name}, the default.`;
  if (custom == null) return [...warning, el("p", {}, byDefault)];
  const { state, name, downloaded, total, sha256, inUse } = custom;
  const say = (text) => (customError ? warningBlock(customError) : el("p", {}, text));
  const progress = () => {
    bar.set(downloaded, total);
    return bar.node;
  };
  const soFar = `${formatSize(downloaded)} of ${formatSize(total)}`;

  if (state === "downloading") {
    return [
      say(`Downloading ${name}… ${soFar}`),
      progress(),
      el("div", { className: "actions" }, button("Pause", pauseDescribingModel), button("Cancel", removeDescribingModel)),
    ];
  }
  if (state === "paused") {
    return [
      say(`Download of ${name} paused at ${soFar}.`),
      progress(),
      el(
        "div",
        { className: "actions" },
        button("Resume", () => downloadDescribingModel(), true),
        button("Cancel", removeDescribingModel),
      ),
    ];
  }
  if (state === "checking") {
    return [say(`Trying ${name} on a sample file…`), el("div", { className: "actions" }, button("Cancel", removeDescribingModel))];
  }
  return [
    ...warning,
    el("p", {}, name, el("span", { className: "muted" }, ` · ${formatSize(total)}`)),
    el("p", { className: "note" }, "SHA-256: ", el("span", { className: "path" }, sha256 ?? "")),
    el("p", {}, inUse ? `Your files are described by ${name}.` : byDefault),
    el(
      "div",
      { className: "actions" },
      inUse ? button("Use the default", () => useDescribingModel(false)) : button(`Use ${name}`, () => useDescribingModel(true), true),
      button("Remove", removeDescribingModel),
    ),
  ];
}

/** "Remove", which asks first, as getting the models back means downloading `total` bytes again; a failure says why in `error`. */
function removeModels(total, error) {
  const node = el("div", { className: "actions" });
  const ask = el("button", { type: "button" }, "Remove…");
  const cancel = el("button", { type: "button" }, "Cancel");
  ask.addEventListener("click", () =>
    node.replaceChildren(
      el("span", {}, `Remove the grouping models? Using them again means downloading ${formatSize(total)}.`),
      modelsButton("Remove", removeGroupingModels, error, true),
      cancel,
    ),
  );
  cancel.addEventListener("click", () => node.replaceChildren(ask));
  node.append(ask);
  return node;
}

/** Each model on disk: its name, size and folder, with "Show in Finder"; a failure says why in `error`. */
function modelList(models, error) {
  return el(
    "ul",
    { className: "settings-list" },
    ...models.map(({ name, size, path }) => {
      const reveal = el("button", { type: "button", className: "link" }, `Show in ${FILE_MANAGER}`);
      reveal.addEventListener("click", () => revealInFileManager(path).catch((e) => showStatus(error, describeError(e), true)));
      return el(
        "li",
        {},
        el("span", {}, name, el("span", { className: "muted" }, ` · ${formatSize(size)}`)),
        el("span", { className: "path" }, splitPath(path, pathSeparator).parent),
        reveal,
      );
    }),
  );
}

/** "Delete all Sessions…", which asks first with how many; unavailable with none. It clears History and the Journals, never a file. */
function sessionsSection() {
  const actions = el("div", { className: "actions" });
  const status = el("p", { className: "status" });
  const ask = el("button", { type: "button", disabled: true }, "Delete all Sessions…");
  const cancel = el("button", { type: "button" }, "Cancel");
  const confirm = el("button", { type: "button", className: "primary" }, "Delete");
  let total = 0;

  const refresh = async () => {
    try {
      ({ total } = await listSessions(0, 0));
    } catch (error) {
      showStatus(status, describeError(error), true);
    }
    ask.disabled = total === 0;
    actions.replaceChildren(ask);
  };
  ask.addEventListener("click", () => {
    showStatus(status, "");
    confirm.disabled = false;
    actions.replaceChildren(
      el("span", {}, `Delete ${total === 1 ? "the 1 Session" : `all ${plural(total, "Session")}`}? You won't be able to undo them from the app. Your files won't change.`),
      confirm,
      cancel,
    );
  });
  cancel.addEventListener("click", refresh);
  confirm.addEventListener("click", async () => {
    confirm.disabled = true;
    try {
      const deleted = await deleteAllSessions();
      showStatus(status, `Deleted ${plural(deleted, "Session")}.`);
    } catch (error) {
      showStatus(status, describeError(error), true);
    }
    refresh();
  });
  refresh();

  return section(
    "History",
    el("p", { className: "note" }, `Deleting the Sessions clears History. Your files stay where they are, and anything in the ${trashPlace(pathSeparator)} can still be put back from there.`),
    actions,
    status,
  );
}
