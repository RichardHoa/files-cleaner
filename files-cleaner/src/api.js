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
