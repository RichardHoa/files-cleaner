// Loaded once, saved on every change.
import { onboarding, saveOnboarding } from "../api.js";

let state = null;

/** If it can't be loaded it counts as seen, so nothing nags. */
export function seen() {
  state ??= onboarding().catch(() => ({ introductionSeen: true, multiSelectSeen: true }));
  return state;
}

export async function updateSeen(change) {
  const next = change(await seen());
  state = Promise.resolve(next);
  try {
    await saveOnboarding(next);
  } catch {
    // Only the memory of it is lost: it shows again next time.
  }
}
