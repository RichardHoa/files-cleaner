# 02: Step flow: Scan report, duplicates, rescan, Suggested Folders

**What to build:** The app walks the user through the pipeline in order. The Scan results screen offers the duplicate step. After the duplicate Session runs, or when the user skips the step or there are no duplicates, the app rescans the Scan Root by itself and goes on to the Suggested Folders step. Until Suggested Folders exist, that step is a placeholder ("Suggested Folders are coming") with a way back to Home.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Scan results: the primary button is "Find duplicates" (was "Build Plan")
- [ ] When the duplicate Plan is empty, the user sees "No duplicates" and can go straight on
- [ ] The duplicate Plan screen has "Skip" next to approve and cancel; Skip goes on without running anything
- [ ] After a duplicate Session finishes, its Session review has "Continue": a fresh scan of the same Scan Root (with progress), then the Suggested Folders step
- [ ] Grouping never uses the scan from before the duplicate Session
- [ ] The duplicate Session is undoable on its own from History, as today
- [ ] `npm test` passes; tests cover the step order and the rescan after a Session or Skip
