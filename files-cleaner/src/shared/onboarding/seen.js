// What the user has seen, the introduction and how to select several items: loaded once, saved on every change.
import { onboarding, saveOnboarding } from "../api.js";

let state = null;

/** Resolves to what the user has seen; if it can't be loaded, it counts as seen, so nothing nags. */
export function seen() {
  state ??= onboarding().catch(() => ({ introductionSeen: true, multiSelectSeen: true }));
  return state;
}

/** Applies `change` to what the user has seen and saves it. */
export async function updateSeen(change) {
  const next = change(await seen());
  state = Promise.resolve(next);
  try {
    await saveOnboarding(next);
  } catch {
    // Only the memory of it is lost: it shows again next time the app opens.
  }
}
