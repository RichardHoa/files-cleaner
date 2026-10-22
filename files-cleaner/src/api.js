// The Tauri commands, one function each. They reject with
// `{ message, configPath, line, column }` (see `CommandError` in src-tauri).
const { invoke, Channel } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const opener = window.__TAURI__.opener;
const events = window.__TAURI__.event;

/** The OS's path separator: "/" on macOS and Linux, "\" on Windows. */
export const pathSeparator = window.__TAURI__.path.sep();

export const loadConfig = () => invoke("load_config");
export const openConfig = () => invoke("open_config");

/** Why the user should think twice before scanning `scanRoot` (it is, or is inside, a Protected Path), or null. */
export const scanRootWarning = (scanRoot) => invoke("scan_root_warning", { scanRoot });

/** Scans `scanRoot`, calling `onProgress({ found, folders, skipped, current })` as it goes. */
export function scan(scanRoot, onProgress) {
  const channel = new Channel();
  channel.onmessage = onProgress;
  return invoke("scan", { scanRoot, onProgress: channel });
}

/**
 * Plans the duplicate step for the latest scan, with the config as it is now: every non-Keeper copy
 * trashed, and the folders that empties offered unticked. `{ root, trashDestination, operations }`, each
 * Operation `{ id, kind, source, relativePath, destination, destinationPath, size, modified, isPackage,
 * isFolder, reason, ticked, error }`, `kind` being "trash", "move" or "createFolder", `reason` one of
 * `{ type: "duplicate", keeper }`, `{ type: "rule", rule }` or `{ type: "emptiedFolder" }`, `ticked` whether
 * it starts ticked, and `error` why it can't happen (it can't be ticked then), or null.
 */
export const buildPlan = () => invoke("build_plan");

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

// The Grouping Models' status is `{ state, downloaded, total, error }`, `state` being "notInstalled",
// "downloading", "paused" or "installed", `downloaded` and `total` in bytes, and `error` why the last
// download failed, or null.

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

/** The folder the user picked, or null if they cancelled. */
export const pickFolder = () => dialog.open({ directory: true, multiple: false, title: "Pick a folder to clean" });

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
