// The Tauri commands, one function each. They reject with
// `{ message, configPath, line, column }` (see `CommandError` in src-tauri).
const { invoke, Channel } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const opener = window.__TAURI__.opener;

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

/** Plans the latest scan with the config as it is now: `{ root, operations, before, after }`. */
export const buildPlan = () => invoke("build_plan");

/** Runs the ticked Operations of the latest Plan as a new Session: `{ sessionId, done, skipped, failed }`. */
export const runPlan = (approved) => invoke("run_plan", { approved });

/** Undoes Session `sessionId`: `{ sessionId, restored, skipped, unrecoverable }`. */
export const undo = (sessionId) => invoke("undo", { sessionId });

/** Undoes only `operationIds` of Session `sessionId`: `{ sessionId, restored, skipped, unrecoverable }`. */
export const undoOperations = (sessionId, operationIds) => invoke("undo_operations", { sessionId, operationIds });

/** Carries out again the undone `operationIds` of Session `sessionId`: `{ sessionId, redone, skipped }`. */
export const redoOperations = (sessionId, operationIds) => invoke("redo_operations", { sessionId, operationIds });

/** Session `sessionId` and its Operations: `{ info, operations: [{ id, kind, source, destination, size, state, reason }] }`. */
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

/** The folder the user picked, or null if they cancelled. */
export const pickFolder = () => dialog.open({ directory: true, multiple: false, title: "Pick a folder to clean" });

/** Opens a Finder window on the folder holding `path`, with it selected. A Package is selected, not opened. */
export const revealInFinder = (path) => opener.revealItemInDir(path);

/** A command error as one line, with its config position when it has one. */
export function describeError(error) {
  if (typeof error === "string") return error;
  if (error.line != null) return `${error.configPath}, line ${error.line}, column ${error.column}: ${error.message}`;
  if (error.configPath) return `${error.configPath}: ${error.message}`;
  return error.message;
}
