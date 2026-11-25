// The only way settings change: the config file is never edited by hand. Headings use plain words;
// the code keeps the glossary's terms.
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
import { el, progressBar, showStatus } from "../../shared/dom.js";
import { formatSize, plural, splitPath } from "../../shared/format.js";
import { showIntroduction } from "../../shared/introduction.js";
import { groupingModelsControls, modelsButton, watchGroupingModels } from "../../shared/models-panel.js";
import { warningBlock } from "../../shared/outcomes.js";

const FILE_MANAGER = pathSeparator === "\\" ? "Explorer" : /Mac/.test(navigator.userAgent) ? "Finder" : "File Manager";

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
  showConfigSections(configSections, nav);
}

function helpSection() {
  const replay = el("button", { type: "button" }, "Replay introduction");
  replay.addEventListener("click", () => showIntroduction());
  return section("Help", el("div", { className: "actions" }, replay));
}

function iconCredit() {
  const author = el("button", { type: "button", className: "link" }, "juicy_fish");
  author.addEventListener("click", () => openWebPage("https://www.flaticon.com/authors/juicy-fish"));
  return el("p", { className: "note" }, "App icon by ", author, " on Flaticon.");
}

/** An unreadable config can only be fixed by resetting it, so that is offered here. */
async function showConfigSections(node, nav) {
  let config;
  try {
    ({ config } = await loadConfig());
  } catch (error) {
    const reset = el(
      "button",
      { type: "button", className: "primary", title: "Puts every setting back to its default, including the Never-Touch Paths." },
      "Reset settings",
    );
    const status = el("p", { className: "status" });
    reset.addEventListener("click", async () => {
      reset.disabled = true;
      try {
        await resetSettings();
        showConfigSections(node, nav);
      } catch (e) {
        showStatus(status, describeError(e), true);
        reset.disabled = false;
      }
    });
    node.replaceChildren(
      el("p", { className: "status error" }, `The settings couldn't be read: ${describeError(error)}`),
      el("div", { className: "actions" }, reset),
      status,
    );
    return;
  }
  node.replaceChildren(
    neverTouchSection(config.neverTouch),
    duplicatesSection(config.defaultCopyRule),
    groupingSection(config.grouping),
    rulesSection(config.rules, nav),
    notificationsSection(config.notifications),
  );
}

const section = (title, ...children) => el("section", { className: "settings-section" }, el("h2", {}, title), ...children);

const choice = (name, label, detail, props) =>
  el(
    "label",
    { className: "settings-choice" },
    el("input", { type: "radio", name, ...props }),
    el("span", {}, label, el("span", { className: "muted" }, ` · ${detail}`)),
  );

function duplicatesSection(initial) {
  const status = el("p", { className: "status" });
  const choices = el(
    "fieldset",
    { className: "settings-choices" },
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
  return section("When files are identical, keep…", choices, status);
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
  return el(
    "section",
    { className: "settings-section" },
    el("h2", { title: "Everything inside them is left alone too. System and app folders are left alone already." }, "Folders to leave alone"),
    list,
    el("div", { className: "actions" }, add),
    status,
  );
}

function rulesSection(rules, nav) {
  const open = el("button", { type: "button" }, "Edit Rules…");
  open.addEventListener("click", () => nav.rules("settings"));
  return section("Rules", el("p", { className: "muted" }, plural(rules.length, "Rule")), el("div", { className: "actions" }, open));
}

function groupingSection(grouping) {
  return section(
    "Folder suggestions",
    folderLevelSetting(grouping.folderThreshold),
    existingFolderSetting(grouping.existingFolderThreshold),
    smallestFolderSetting(grouping.smallestFolder),
  );
}

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
 * A stored value between levels is snapped to the nearest and saved, so what is shown is what
 * applies.
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
    el(
      "div",
      { className: "settings-setting-head" },
      el("strong", { title: "How often a suggested folder goes into a folder already in the scanned folder, rather than a new one." }, "Use your existing folders"),
      reset,
    ),
    choices,
    error,
  );
}

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
    status,
  );
}

function readAgainSection() {
  const button = el(
    "button",
    { type: "button", title: "Forgets what was read from your files before. It takes as long as the first time." },
    "Read every file again",
  );
  const status = el("p", { className: "status" });
  button.addEventListener("click", async () => {
    button.disabled = true;
    try {
      await clearGroupingCards();
      showStatus(status, "Done");
    } catch (error) {
      showStatus(status, describeError(error), true);
      button.disabled = false;
    }
  });
  return el(
    "div",
    { className: "settings-setting" },
    el("div", { className: "actions" }, button),
    status,
  );
}

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
      ...(installed ? [el("p", { title: "Files are grouped by what's inside them." }, "Installed")] : groupingModelsControls(status, error, bar)),
      ...(onDisk.length ? [modelList(onDisk, error), el("p", { className: "muted" }, `Total: ${formatSize(total)}`)] : []),
      installed ? removeModels(status.total, error) : "",
    );
    showStatus(error, "");
  });
  return node;
}

/** Built once and fed each status, so what's typed in the box stays. */
function describingModelSetting() {
  const link = el("input", {
    type: "url",
    className: "settings-link",
    placeholder: "Hugging Face .gguf link",
    ariaLabel: "Hugging Face link to one .gguf file",
    title:
      "Describe your files with another chat model: the link to one .gguf file. It's tried on a sample file first. " +
      "The model that compares the descriptions stays the same.",
  });
  const download = el("button", { type: "button" }, "Download");
  const current = el("div");
  const error = el("p", { className: "status error" });
  const bar = progressBar();
  const node = el(
    "details",
    { className: "settings-setting" },
    el("summary", {}, "Advanced"),
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

/** Asks first, as getting the models back means a long download. */
function removeModels(total, error) {
  const node = el("div", { className: "actions" });
  const ask = el("button", { type: "button" }, "Remove…");
  const cancel = el("button", { type: "button" }, "Cancel");
  ask.addEventListener("click", () =>
    node.replaceChildren(
      el("span", {}, `Remove the grouping models? Re-download is ${formatSize(total)}.`),
      modelsButton("Remove", removeGroupingModels, error, true),
      cancel,
    ),
  );
  cancel.addEventListener("click", () => node.replaceChildren(ask));
  node.append(ask);
  return node;
}

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

/** Asks first, as past Sessions can't be undone afterwards. */
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
      el("span", {}, `Delete ${total === 1 ? "the 1 Session" : `all ${plural(total, "Session")}`}? Your files stay put.`),
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
    actions,
    status,
  );
}

// Levels instead of numbers, as a threshold means nothing to the user. The config keeps thresholds,
// so a value between levels shows at the nearest.

/** Looser means fewer, broader folders. */
export const FOLDER_LEVELS = [
  { label: "A few broad folders", threshold: 0.8 },
  { label: "Fewer, broader folders", threshold: 0.83 },
  { label: "Some folders", threshold: 0.86 },
  { label: "More, narrower folders", threshold: 0.89 },
  { label: "Many specific folders", threshold: 0.92 },
];

/** Looser first: files go into existing folders more often. */
export const EXISTING_FOLDER_LEVELS = [
  { label: "Often", threshold: 0.82 },
  { label: "Sometimes", threshold: 0.86 },
  { label: "Rarely", threshold: 0.9 },
];

/** The looser level on a tie. */
export function nearestLevel(levels, value) {
  let nearest = 0;
  for (const [index, level] of levels.entries()) {
    if (Math.abs(level.threshold - value) < Math.abs(levels[nearest].threshold - value)) nearest = index;
  }
  return nearest;
}

/** Approximate, as the config stores 32-bit floats. */
export const onLevel = (levels, value) => Math.abs(levels[nearestLevel(levels, value)].threshold - value) < 1e-4;
