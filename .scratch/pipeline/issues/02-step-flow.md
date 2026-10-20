# 02: Step flow: Scan report, duplicates, rescan, Suggested Folders

**What to build:** The app walks the user through the pipeline in order. The Scan results screen offers the duplicate step. After the duplicate Session runs, or when the user skips the step or there are no duplicates, the app rescans the Scan Root by itself and goes on to the Suggested Folders step. Until Suggested Folders exist, that step is a placeholder ("Suggested Folders are coming") with a way back to Home.

**Blocked by:** 01

**Status:** resolved

- [x] Scan results: the primary button is "Find duplicates" (was "Build Plan")
- [x] When the duplicate Plan is empty, the user sees "No duplicates" and can go straight on
- [x] The duplicate Plan screen has "Skip" next to approve and cancel; Skip goes on without running anything
- [x] After a duplicate Session finishes, its Session review has "Continue": a fresh scan of the same Scan Root (with progress), then the Suggested Folders step
- [x] Grouping never uses the scan from before the duplicate Session
- [x] The duplicate Session is undoable on its own from History, as today
- [x] `npm test` passes; tests cover the step order and the rescan after a Session or Skip

## Comments

The order lives in `src/steps.js` (`stepFlow`, given `buildPlan` and `scan` from main.js so `tests/steps.test.js` can drive it with fakes). Scan results → "Find duplicates" → the Plan screen ("No duplicates" + Continue when empty; otherwise Cancel / Skip / Move). Running opens the Session review with `fromDuplicateStep(scanRoot)`, which adds "Continue". Continue and Skip both rescan the Scan Root with progress, then show `screens/suggested-folders.js`, the placeholder, with that fresh scan. The backend's latest scan is replaced by the rescan, so the old scan is never planned again. History and Undo are unchanged.
