# 07: One review screen, one Plan, one Session

**What to build:** After the scan, the user goes straight to one screen holding the whole Plan, approves it once, and it runs as one Session (ADR 0005).
- The Scan results screen, the separate duplicate Plan screen, the duplicate Session's "Continue" and the rescan are gone. A one-line scan summary (items, total size) sits in the review screen's header.
- The sidebar lists Duplicates, then each Rule, then Left where they are, then each Suggested Folder, then Emptied Folders, each with a count.
- Duplicates are shown as Duplicate Groups: file name, number of copies and space freed. Every copy is listed with its full folder. The Keeper (the default rule, unchanged) comes first, marked **Keep**, and the other copies are marked **Trash**. Right-click shows a copy in Finder. There is no right-hand explanation pane, and the word "Original" is gone.
- Duplicates and Rules show as soon as the scan ends. Reading starts at once from that same scan, on every loose file except copies marked Trash and files matched by a Rule. The Suggested Folders section shows the reading progress until it finishes.
- Emptied Folders are worked out from the whole Plan, folder moves included, and are unticked as now.
- Approve says "Waiting for folders…" and stays disabled until reading finishes. It then builds one Plan (Duplicate, Rule, Suggested Folder and Emptied Folder Reasons) and runs it as one Session, shown once in History and undoable as a whole or per Operation.
- Cancel stops any reading, throws the Plan away and goes Home. Saved readings are kept.
- Without the Grouping Models, the screen has the Duplicates and Rule sections plus the models offer.

See `../one-screen-review.md`: user stories 1–12, 18–21, 27–32, 36, 53, 56.

**Blocked by:** None (can start immediately). Doing 06 first avoids conflicts on the Suggested Folders screen.

**Status:** resolved

- [x] The step flow is pick folder → scan → review; Scan results, the duplicate screen and the rescan are removed
- [x] The backend reports Duplicate Groups (every copy, sizes, dates, the default Keeper) and builds one Plan from the duplicate choices, Rule Operations, Suggested Folder choices and the resulting Emptied Folders
- [x] Reading uses the first scan and skips copies marked Trash and files matched by a Rule
- [x] Merged screen with the sidebar order above; the Duplicates pane lists Duplicate Groups with Keep / Trash
- [x] Approve waits for reading, then runs one Session; Cancel stops reading and goes Home with nothing changed
- [x] Without the models: Duplicates, Rules and the models offer
- [x] Quiet native styling; no "AI" wording
- [x] Test: picking a folder scans once and shows the review, with no Scan results screen and no second scan (replaces the old step flow tests)

## Comments

- 2026-10-29: Done, without Rules: they stay frozen (decided with the developer), so the Plan holds Duplicate, Suggested Folder and Emptied Folder Reasons only, the sidebar has no Rule entries, and reading skips only the copies marked Trash. `find_duplicates` reports the Duplicate Groups (default Keeper first) and keeps them, so reading leaves out every other copy; `build_review_plan` takes `{ keeper, trash }` per group plus the Suggested Folder choices and builds the one Plan. The Emptied Folders are worked out by the backend again a moment after each change, so their sidebar count stays right; the ticked ones are carried into Approve. If reading fails, the error shows in the Suggested Folders' place and Approve runs the duplicates alone. `suggested-choices.js` is now `review-choices.js`; Scan results, the duplicate Plan screen, Continue and the rescan are gone.
