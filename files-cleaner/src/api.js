// The Tauri commands, one function each. They reject with
// `{ message, configPath, line, column }` (see `CommandError` in src-tauri).
const { invoke, Channel } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const opener = window.__TAURI__.opener;
const events = window.__TAURI__.event;
// On the desktop the plugin never asks for permission: the app asks the user itself (see `notifications`).
const notification = window.__TAURI__.notification;
const appWindow = window.__TAURI__.window;

/** The OS's path separator: "/" on macOS and Linux, "\" on Windows. */
export const pathSeparator = window.__TAURI__.path.sep();

/**
 * The config: `{ path, config }`, `config` being `{ home, trashDestination, neverTouch, rules, grouping, notifications }` and
 * `grouping` `{ folderThreshold, existingFolderThreshold, smallestFolder }`.
 */
export const loadConfig = () => invoke("load_config");

/**
 * Saves one change to the config file, keeping everything else in it, and resolves to the config as
 * `loadConfig` does. `key` is "neverTouch" (the whole list of paths), "folderThreshold",
 * "existingFolderThreshold", "smallestFolder" or "notifications" (whether the user wants them); a grouping
 * `value` of null resets it to the default.
 * Rejects, saving nothing, if the value is invalid.
 */
export const changeSetting = (key, value) => invoke("change_setting", { change: { key, value } });

/** Puts every setting back to its default, even when the config can't be read; resolves as `loadConfig` does. */
export const resetSettings = () => invoke("reset_settings");

/** Why the user should think twice before scanning `scanRoot` (it is, or is inside, a Protected Path), or null. */
export const scanRootWarning = (scanRoot) => invoke("scan_root_warning", { scanRoot });

/** Scans `scanRoot`, calling `onProgress({ found, folders, skipped, current })` as it goes. */
export function scan(scanRoot, onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("scan", { scanRoot, onProgress: channel });
}

/**
 * The Duplicate Groups of the latest scan: `[{ size, copies }]`, each copy `{ path, relativePath, modified,
 * created }`, the default Keeper first. Calls `onProgress({ phase, finished, total, current })` while finding
 * them: `phase` "comparing" counts files whose start and end are compared, then "checking" counts the bytes
 * of those read in full. Reading for Suggested Folders then leaves out every copy but the Keeper.
 */
export function findDuplicates(onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("find_duplicates", { onProgress: channel });
}

// Running, undoing and redoing resolve to a summary: `{ sessionId, done, skipped, failed, unrecoverable }`,
// each a list of `{ operationId, kind, source, destination, note }`, the note saying why it wasn't done.

/** Runs the ticked Operations of the latest Plan as a new Session, calling `onProgress({ finished, total, current })` as it goes. */
export function runPlan(approved, onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("run_plan", { approved, onProgress: channel });
}

/** Undoes the done `operationIds` of Session `sessionId`, or every done Operation if they're left out. */
export const undo = (sessionId, operationIds) => invoke("undo", { sessionId, operationIds });

/** Carries out again the undone `operationIds` of Session `sessionId`. */
export const redoOperations = (sessionId, operationIds) => invoke("redo_operations", { sessionId, operationIds });

/**
 * Session `sessionId` and its Operations: `{ info, operations }`, each Operation
 * `{ id, kind, source, destination, relativePath, relativeDestination, size, state, note }`.
 */
export const sessionDetails = (sessionId) => invoke("session_details", { sessionId });

/**
 * Up to `limit` past Sessions, newest first, after skipping `offset`: `{ total, sessions }`, each Session
 * `{ id, scanRoot, startedAt, status, done, restored, leftToUndo, totalSize, kindCounts: { trash: 3, … } }`.
 */
export const listSessions = (offset, limit) => invoke("list_sessions", { offset, limit });

/** The Recent Scan Roots, newest first: `[{ path, lastScanned, exists }]`. Scanning a folder adds it. */
export const recentScanRoots = () => invoke("recent_scan_roots");

/** Takes `scanRoot` off the Recent Scan Roots. */
export const removeRecentScanRoot = (scanRoot) => invoke("remove_recent_scan_root", { scanRoot });

// The Grouping Models' status is `{ state, downloaded, total, error, models }`, `state` being
// "notInstalled", "downloading", "paused" or "installed", `downloaded` and `total` in bytes, `error` why the
// last download failed, or null, and `models` each model's `{ name, size, path }`: its bytes on disk and
// its file (the partial one while downloading), or null when nothing of it is on disk.

export const groupingModelsStatus = () => invoke("grouping_models_status");

/** Starts or resumes the download in the background; rejects if the disk is too full for it. */
export const downloadGroupingModels = () => invoke("download_grouping_models");

/** Pauses the download, keeping what's downloaded so far. */
export const pauseGroupingModels = () => invoke("pause_grouping_models");

/** Cancels the download, or removes the installed Grouping Models. */
export const removeGroupingModels = () => invoke("remove_grouping_models");

/** Calls `onStatus(status)` on every change; resolves to a function that stops listening. */
export const onGroupingModelsStatus = (onStatus) => events.listen("grouping-models", (event) => onStatus(event.payload));

/** Whether to offer the Grouping Models before Home: until the offer is answered, unless they're installed. */
export const groupingModelsOfferPending = () => invoke("grouping_models_offer_pending");

/** Remembers the answer to the first-open offer, "download" or "notNow", so it isn't asked again. */
export const answerGroupingModelsOffer = (answer) => invoke("answer_grouping_models_offer", { answer });

// The Suggested Folders are `{ folders, ungrouped, summaries }`: `folders` largest first, each
// `{ name, files, existing, fixedKind }`, `existing` the matched subfolder's path or null; `ungrouped` the
// files left where they are; `summaries` each read file's one-sentence summary, keyed by path.

/**
 * Reads the loose files of the latest scan with the Grouping Models and groups them by the config's
 * `grouping` settings, calling `onProgress({ finished, total, current, placing, loading })` as it goes:
 * `total` counts only the files not read before, `loading` is set until the first of them is read, and
 * `placing` once the files left out of every folder are being placed. Resolves to `{ grouping, device }`,
 * `device` where the models ran or null. Rejects once `cancelAnalysis` stops it.
 */
export function analyseLooseFiles(onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("analyse_loose_files", { onProgress: channel });
}

/** Stops the running analysis; the files read so far aren't read again next time. */
export const cancelAnalysis = () => invoke("cancel_analysis");

/** Ends the running analysis's reading between files: `analyseLooseFiles` then resolves with the files read so far grouped and the rest left where they are. */
export const stopReading = () => invoke("stop_reading");

/** Forgets what the Grouping Models said about every file, so the next analysis reads them all again. */
export const clearGroupingCards = () => invoke("clear_grouping_cards");

/** The seconds a file took to read on this computer, averaged over finished analyses (see `updatedAverage`), or null before the first. */
export const readingPace = () => invoke("reading_pace");

/** Remembers `secondsPerFile` as this computer's reading pace. */
export const saveReadingPace = (secondsPerFile) => invoke("save_reading_pace", { secondsPerFile });

/** Whether the user wants notifications, from the config: true, false, or null until they're asked. */
export const notifications = async () => (await loadConfig()).config.notifications ?? null;

/** Keeps the user's answer about notifications in the config, so they're never asked again. */
export const answerNotifications = (wanted) => changeSetting("notifications", wanted);

/** Sends a system notification saying `title`, only while the app window isn't focused and the user wants them. */
export async function notifyInBackground(title) {
  if ((await notifications()) !== true || (await appWindow.getCurrentWindow().isFocused())) return;
  notification.sendNotification({ title });
}

/**
 * Plans the whole review over the latest scan as one Plan. `duplicates` is `[{ keeper, trash }]`, the path
 * of the copy that stays and the paths of those trashed. `folders` is `[{ name, choice, files }]`, `choice`
 * one of `{ type: "createHere" }`, `{ type: "addTo", folder }`, `{ type: "createIn", folder }`,
 * `{ type: "trashAll" }` or `{ type: "skip" }`, and `files` `[[path, "inFolder" | "trash" | "takeOut"]]`.
 * Resolves to `{ root, trashDestination, operations }`, each Operation `{ id, kind, source, relativePath,
 * destination, destinationPath, size, modified, isPackage, isFolder, reason, ticked, error }`, `kind` being
 * "trash", "move" or "createFolder", `reason` one of `{ type: "duplicate", keeper }`,
 * `{ type: "suggestedFolder", folder }` or `{ type: "emptiedFolder" }`, `ticked` whether it starts ticked
 * (an emptied folder doesn't), and `error` why it can't happen (it can't be ticked then), or null.
 * Rejects if a name can't be a folder name.
 */
export const buildReviewPlan = (duplicates, folders) => invoke("build_review_plan", { duplicates, folders, keep: true });

/** Plans as `buildReviewPlan` does, only to look at: `runPlan` still runs the Plan built before. */
export const previewReviewPlan = (duplicates, folders) => invoke("build_review_plan", { duplicates, folders, keep: false });

/** The folder the user picked, or null if they cancelled. */
export const pickFolder = (title = "Pick a folder to clean") => dialog.open({ directory: true, multiple: false, title });

/**
 * Opens the file manager (Finder, Explorer or the Linux one) on the folder holding `path`, with it selected. A Package is selected, not opened.
 * Given a list of paths, selects them all, with a window for each folder they are in.
 */
export const revealInFileManager = (path) => opener.revealItemInDir(path);

/** A command error as one line, with its config position when it has one. */
export function describeError(error) {
  if (typeof error === "string") return error;
  if (error.line != null) return `${error.configPath}, line ${error.line}, column ${error.column}: ${error.message}`;
  if (error.configPath) return `${error.configPath}: ${error.message}`;
  return error.message;
}
