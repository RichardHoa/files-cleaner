// Payload shapes are the Rust types serialised in src-tauri. All reject with its `CommandError`.
const { invoke, Channel } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const opener = window.__TAURI__.opener;
const events = window.__TAURI__.event;
// On the desktop the plugin never asks for permission, so the app asks the user itself.
const notification = window.__TAURI__.notification;
const appWindow = window.__TAURI__.window;

export const pathSeparator = window.__TAURI__.path.sep();

export const loadConfig = () => invoke("load_config");

/** A grouping `value` of null resets it. Nothing is saved if the value is invalid. */
export const changeSetting = (key, value) => invoke("change_setting", { change: { key, value } });

/** Works even when the config can't be read. */
export const resetSettings = () => invoke("reset_settings");

export const scanRootWarning = (scanRoot) => invoke("scan_root_warning", { scanRoot });

// Lets a Cancel reach a scan that hasn't started yet. Starts from the time, so a reloaded page
// never reuses a cancelled scan's ID.
let scanId = Date.now();

export function scan(scanRoot, onProgress) {
  scanId += 1;
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("scan", { scanRoot, scanId, onProgress: channel });
}

/** Also stops the search for duplicates after it. */
export const cancelScan = () => invoke("cancel_scan", { scanId });

/** "comparing" progress counts files, "checking" counts bytes. */
export function findDuplicates(onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("find_duplicates", { onProgress: channel });
}

// These resolve to a `Summary`, whose `sessionId` is null after a Run that did nothing.

// Like `scanId`.
let runId = Date.now();

/** A cancelled Run resolves to the put-back's `Summary`, with `cancelled` set. */
export function runPlan(approved, onProgress) {
  runId += 1;
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("run_plan", { approved, runId, onProgress: channel });
}

/** False once the Run has finished: too late to put anything back. */
export const cancelRun = () => invoke("cancel_run", { runId });

/** All of them if `operationIds` is null. Progress comes as a Run's does. */
export function undo(sessionId, operationIds, onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("undo", { sessionId, operationIds, onProgress: channel });
}

export function redoOperations(sessionId, operationIds, onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("redo_operations", { sessionId, operationIds, onProgress: channel });
}

export function retryOperations(sessionId, operationIds, onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("retry_operations", { sessionId, operationIds, onProgress: channel });
}

export const sessionDetails = (sessionId) => invoke("session_details", { sessionId });

export const listSessions = (offset, limit) => invoke("list_sessions", { offset, limit });

/** Changes no file of the user's. Refused during a Run, Undo or Redo. */
export const deleteAllSessions = () => invoke("delete_all_sessions");

export const recentScanRoots = () => invoke("recent_scan_roots");

export const removeRecentScanRoot = (scanRoot) => invoke("remove_recent_scan_root", { scanRoot });

// `state` is "notInstalled", "downloading", "paused" or "installed"; `custom` is null unless the
// user picked a model.

export const groupingModelsStatus = () => invoke("grouping_models_status");

export const downloadGroupingModels = () => invoke("download_grouping_models");

export const pauseGroupingModels = () => invoke("pause_grouping_models");

export const removeGroupingModels = () => invoke("remove_grouping_models");

/** A null `link` resumes the stopped download. Used only once it passes its check. */
export const downloadDescribingModel = (link = null) => invoke("download_describing_model", { link });

export const pauseDescribingModel = () => invoke("pause_describing_model");

export const removeDescribingModel = () => invoke("remove_describing_model");

/** Nothing is downloaded either way. */
export const useDescribingModel = (picked) => invoke("use_describing_model", { picked });

export const onGroupingModelsStatus = (onStatus) => events.listen("grouping-models", (event) => onStatus(event.payload));

export const onboarding = () => invoke("onboarding");

export const saveOnboarding = (state) => invoke("save_onboarding", { onboarding: state });

// `basis` is "topic", "kind", "existing" or "rule".

/** No model runs, so these can be shown while the rest are read. */
export const ruleFolders = () => invoke("rule_folders");

export const ruleOverlaps = (rules) => invoke("rule_overlaps", { rules });

/** Without `read`, it groups as if stopped at once. Rejects once `cancelAnalysis` stops it. */
export function analyseLooseFiles(onProgress, { read = true } = {}) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("analyse_loose_files", { read, onProgress: channel });
}

/** 0 without the Grouping Models. Rejects once `cancelAnalysis` stops it. */
export const filesToRead = () => invoke("files_to_read");

/** The files read so far aren't read again next time. */
export const cancelAnalysis = () => invoke("cancel_analysis");

/** Unlike cancelling, the files read so far are still grouped. */
export const stopReading = () => invoke("stop_reading");

export const clearGroupingCards = () => invoke("clear_grouping_cards");

/** Null before the first finished analysis. */
export const readingPace = () => invoke("reading_pace");

export const saveReadingPace = (secondsPerFile) => invoke("save_reading_pace", { secondsPerFile });

/** Null until the user is asked. */
export const notifications = async () => (await loadConfig()).config.notifications ?? null;

export const answerNotifications = (wanted) => changeSetting("notifications", wanted);

/** Not while the app is focused: the user already sees it. */
export async function notifyInBackground(title) {
  if ((await notifications()) !== true || (await appWindow.getCurrentWindow().isFocused())) return;
  notification.sendNotification({ title });
}

/**
 * Rejects if a name can't be a folder name. A Trash has no destination: only the OS knows where
 * it lands.
 */
export const buildReviewPlan = (duplicates, folders) => invoke("build_review_plan", { duplicates, folders, keep: true });

/** Only to look at: `runPlan` still runs the Plan built before. */
export const previewReviewPlan = (duplicates, folders) => invoke("build_review_plan", { duplicates, folders, keep: false });

export const pickFolder = (title = "Pick a folder to clean") => dialog.open({ directory: true, multiple: false, title });

/** Such files can't be moved there. */
export const onAnotherDisk = (paths, folder) => invoke("on_another_disk", { paths, folder });

/** A Package is selected, not opened. */
export const revealInFileManager = (path) => opener.revealItemInDir(path);

export const openWebPage = (url) => opener.openUrl(url);

export const openFile = (path) => invoke("open_file", { path });

export const openSessionFile = (sessionId, operationId) => invoke("open_session_file", { sessionId, operationId });

export const setMenuState = (state) => invoke("set_menu_state", { state });

export const onMenuClick = (onClick) => events.listen("menu", (event) => onClick(event.payload));

export const showContextMenu = (items) => invoke("show_context_menu", { items });

export const onContextMenuChoice = (onChosen) => events.listen("context-menu", (event) => onChosen(event.payload));

export const askToConfirm = (question, okLabel) => dialog.ask(question, { okLabel, cancelLabel: "Cancel", kind: "info" });

export function describeError(error) {
  if (typeof error === "string") return error;
  if (error.line != null) return `${error.configPath}, line ${error.line}, column ${error.column}: ${error.message}`;
  if (error.configPath) return `${error.configPath}: ${error.message}`;
  return error.message;
}
