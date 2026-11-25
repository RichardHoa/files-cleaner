import { describeError } from "./api.js";

export function showStatus(node, text, isError = false) {
  node.textContent = text;
  node.classList.toggle("error", isError);
  node.classList.remove("warning-block");
}

/** Replaces the progress: a whole step failed, not just one action. */
export function showWarning(node, text) {
  showStatus(node, text);
  node.classList.add("warning-block");
}

/** Buttons are disabled so the action can't be started twice. */
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
