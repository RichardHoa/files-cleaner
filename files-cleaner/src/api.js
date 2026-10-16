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

/**
 * Plans the latest scan with the config as it is now: `{ root, trashDestination, operations }`, each
 * Operation `{ id, kind, source, relativePath, destination, destinationPath, size, modified, isPackage, reason }`,
 * `kind` being "trash", "move" or "createFolder" and `reason` either `{ type: "duplicate", keeper }` or
 * `{ type: "rule", rule }`.
 */
export const buildPlan = () => invoke("build_plan");

// Running, undoing and redoing resolve to a summary: `{ sessionId, done, skipped, failed, unrecoverable }`,
// each a list of `{ operationId, kind, source, destination, note }`, the note saying why it wasn't done.

/** Runs the ticked Operations of the latest Plan as a new Session. */
export const runPlan = (approved) => invoke("run_plan", { approved });

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

/** The folder the user picked, or null if they cancelled. */
export const pickFolder = () => dialog.open({ directory: true, multiple: false, title: "Pick a folder to clean" });

/**
 * Opens a Finder window on the folder holding `path`, with it selected. A Package is selected, not opened.
 * Given a list of paths, selects them all, with a window for each folder they are in.
 */
export const revealInFinder = (path) => opener.revealItemInDir(path);

/** A command error as one line, with its config position when it has one. */
export function describeError(error) {
  if (typeof error === "string") return error;
  if (error.line != null) return `${error.configPath}, line ${error.line}, column ${error.column}: ${error.message}`;
  if (error.configPath) return `${error.configPath}: ${error.message}`;
  return error.message;
}
