// Status lines: a one-line message under an action, red when it's an error, or the orange warning when a
// whole step failed.
import { describeError } from "./api.js";

/** Shows `text` in the status line `node`, styled as an error if `isError`. */
export function showStatus(node, text, isError = false) {
  node.textContent = text;
  node.classList.toggle("error", isError);
  node.classList.remove("warning-block");
}

/** Shows `text` in the status line `node`, in place of its progress, as the orange warning: a whole step failed. */
export function showWarning(node, text) {
  showStatus(node, text);
  node.classList.add("warning-block");
}

/**
 * Runs `action` with `buttons` disabled and `label` in the status line `node`, then enables them
 * again. If it fails, the status line says why. Resolves to `action`'s result, or undefined if it failed.
 */
export async function busy(node, buttons, label, action) {
  for (const button of buttons) button.disabled = true;
  showStatus(node, label);
  try {
    return await action();
  } catch (error) {
    showStatus(node, describeError(error), true);
    return undefined;
  } finally {
    for (const button of buttons) button.disabled = false;
  }
}
