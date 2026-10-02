// The Tauri commands, one function each. They reject with
// `{ message, configPath, line, column }` (see `CommandError` in src-tauri).
const { invoke } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;

export const loadConfig = () => invoke("load_config");
export const openConfig = () => invoke("open_config");
export const scan = (scanRoot) => invoke("scan", { scanRoot });

/** The folder the user picked, or null if they cancelled. */
export const pickFolder = () => dialog.open({ directory: true, multiple: false, title: "Pick a folder to clean" });

/** A command error as one line, with its config position when it has one. */
export function describeError(error) {
  if (typeof error === "string") return error;
  if (error.line != null) return `${error.configPath}, line ${error.line}, column ${error.column}: ${error.message}`;
  if (error.configPath) return `${error.configPath}: ${error.message}`;
  return error.message;
}
