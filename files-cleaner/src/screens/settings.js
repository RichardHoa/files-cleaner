// Settings: the Trash Destination, the Never-Touch Paths, how files are grouped, and the Grouping Models.
// This is the only way settings change: the config file behind them is never edited by hand. Each change
// is saved as soon as it's made; an invalid value says why beside it and isn't saved. Grouping changes
// apply the next time files are grouped.
import {
  changeSetting,
  clearGroupingCards,
  describeError,
  loadConfig,
  pathSeparator,
  pickFolder,
  removeGroupingModels,
  resetSettings,
  revealInFileManager,
} from "../api.js";
import { el } from "../dom.js";
import { formatSize, splitPath } from "../format.js";
import { groupingModelsControls, modelsButton, watchGroupingModels } from "../models-panel.js";
import { showStatus } from "../status.js";

/** What this OS calls its file manager, for "Show in Finder". */
const FILE_MANAGER = pathSeparator === "\\" ? "Explorer" : /Mac/.test(navigator.userAgent) ? "Finder" : "File Manager";

/** The two grouping thresholds: what each slider's ends mean, and one line on what it changes. */
const THRESHOLDS = [
  {
    key: "folderThreshold",
    label: "Files share a folder",
    looser: "Looser: fewer, bigger folders",
    stricter: "Stricter: more, smaller folders",
    explain: "How closely files must match to be suggested one folder.",
  },
  {
    key: "existingFolderThreshold",
    label: "Files go into an existing folder",
    looser: "Looser: your folders are reused more often",
    stricter: "Stricter: more new folders",
    explain: "How closely a suggested folder must match one already in the scanned folder to go into it.",
  },
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
  );
  showConfigSections(configSections);
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
  node.replaceChildren(trashSection(), neverTouchSection(config.neverTouch), groupingSection(config.grouping));
}

/** `title`'s section, holding `children`. */
const section = (title, ...children) => el("section", { className: "settings-section" }, el("h2", {}, title), ...children);

function trashSection() {
  const choice = (label, detail, props) =>
    el(
      "label",
      { className: "settings-choice" },
      el("input", { type: "radio", name: "trash-destination", ...props }),
      el("span", {}, label, el("span", { className: "muted" }, ` · ${detail}`)),
    );
  return section(
    "Trash Destination",
    el(
      "fieldset",
      { className: "settings-choices" },
      el("legend", { className: "note" }, "Where trashed files go. Nothing is ever deleted for good."),
      choice("Holding Folder", "a “Files Cleaner Trash” folder inside the folder you clean", { checked: true }),
      choice("System Trash", "not yet supported", { disabled: true }),
    ),
  );
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
    "Never-Touch Paths",
    el("p", { className: "note" }, "Folders the app leaves alone, everything inside them included. System and app folders are left alone already."),
    list,
    el("div", { className: "actions" }, add),
    status,
  );
}

function groupingSection(grouping) {
  return section(
    "Grouping",
    el("p", { className: "note" }, "Changes apply the next time files are grouped."),
    ...THRESHOLDS.map((threshold) => thresholdSetting(threshold, grouping[threshold.key])),
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

/** A threshold: a slider and the exact number, kept in step, with "Reset to default". */
function thresholdSetting({ key, label, looser, stricter, explain }, initial) {
  const slider = el("input", { type: "range", min: 0, max: 1, step: 0.01, ariaLabel: label });
  const number = el("input", { type: "number", min: 0, max: 1, step: 0.01, className: "settings-number", ariaLabel: `${label}, exact` });
  const reset = el("button", { type: "button", className: "link" }, "Reset to default");
  const error = el("p", { className: "status" });
  const show = (value) => {
    slider.value = value;
    number.value = value.toFixed(2);
    number.ariaInvalid = "false";
  };
  const save = async (value) => {
    const saved = await saveGrouping(key, value, error);
    if (saved !== undefined) show(saved);
  };

  slider.addEventListener("input", () => (number.value = Number(slider.value).toFixed(2)));
  slider.addEventListener("change", () => save(Number(slider.value)));
  number.addEventListener("input", () => {
    const value = number.valueAsNumber;
    if (value >= 0 && value <= 1) slider.value = value;
  });
  number.addEventListener("change", () => {
    const value = number.valueAsNumber;
    if (!(value >= 0 && value <= 1)) {
      number.ariaInvalid = "true";
      showStatus(error, "Enter a number from 0 to 1.", true);
      return;
    }
    save(Math.round(value * 100) / 100);
  });
  reset.addEventListener("click", () => save(null));
  show(initial);

  return el(
    "div",
    { className: "settings-setting" },
    el("div", { className: "settings-setting-head" }, el("strong", {}, label), reset),
    el("p", { className: "note" }, explain),
    el(
      "div",
      { className: "settings-slider" },
      el("span", { className: "muted" }, looser),
      slider,
      el("span", { className: "muted" }, stricter),
      number,
    ),
    error,
  );
}

/** The fewest files a Suggested Folder holds, 2–10. */
function smallestFolderSetting(initial) {
  const number = el("input", { type: "number", min: 2, max: 10, step: 1, value: initial, className: "settings-number" });
  const error = el("p", { className: "status" });
  number.addEventListener("change", async () => {
    const value = number.valueAsNumber;
    if (!(Number.isInteger(value) && value >= 2 && value <= 10)) {
      number.ariaInvalid = "true";
      showStatus(error, "Enter a whole number from 2 to 10.", true);
      return;
    }
    const saved = await saveGrouping("smallestFolder", value, error);
    if (saved !== undefined) {
      number.value = saved;
      number.ariaInvalid = "false";
    }
  });
  return el(
    "div",
    { className: "settings-setting" },
    el("label", { className: "settings-setting-head" }, el("strong", {}, "Smallest folder"), number, el("span", { className: "muted" }, "files")),
    el("p", { className: "note" }, "Fewer files than this alike aren't suggested a folder of their own."),
    error,
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

/** The Grouping Models: their status and controls, and each model's name, size on disk and folder. */
function modelsSection() {
  const body = el("div");
  const error = el("p", { className: "status error" });
  const node = section("Grouping Models", body, error);
  watchGroupingModels(node, error, (status) => {
    const onDisk = status.models.filter((model) => model.path);
    const total = onDisk.reduce((sum, model) => sum + model.size, 0);
    const installed = status.state === "installed";
    body.replaceChildren(
      ...(installed ? [el("p", {}, "Installed. Files are grouped by what's inside them.")] : groupingModelsControls(status, error)),
      ...(onDisk.length ? [modelList(onDisk, error), el("p", { className: "muted" }, `Total: ${formatSize(total)}`)] : []),
      installed ? removeModels(status.total, error) : "",
    );
    showStatus(error, status.error ?? "", status.error != null);
  });
  return node;
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
