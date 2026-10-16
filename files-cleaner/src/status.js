// Status lines: a one-line message under an action, red when it's an error.
import { describeError } from "./api.js";

/** Shows `text` in the status line `node`, styled as an error if `isError`. */
export function showStatus(node, text, isError = false) {
  node.textContent = text;
  node.classList.toggle("error", isError);
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
