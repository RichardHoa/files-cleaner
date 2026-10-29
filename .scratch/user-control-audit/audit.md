# User-control audit

Status: needs-triage

Date: 2026-10-29

A read-only pass over the whole app (UI in `files-cleaner/src`, core in `src-tauri/core`, grouping in `src-tauri/grouping`) looking for places where the app decides for the user, hides a decision from them, or offers a choice that is narrower than what they'd plausibly want. Nothing was run; every finding is from the code, with where it lives.

Severity: **High** = blocks a common, reasonable intent. **Medium** = workable but awkward, or a hidden decision the user would want to know about. **Low** = polish.

---

## The five root assumptions

Most findings below come from a handful of assumptions baked into the domain model. Fixing these at the root fixes most findings; patching findings one by one won't.

### R1. "Every item has one right answer"
A Duplicate Group has exactly one **Keeper** (`GLOSSARY.md`, "Keeper"). A file belongs to exactly one Suggested Folder or to "Left where they are". A Suggested Folder has one destination. The UI is built around picking *the* answer, not around the user saying what they want for each item.

### R2. "The app decides what's in scope, silently"
Hidden files, files with no extension, symlinks, 0-byte files and Packages are dropped before the user sees anything. Grouping only considers files sitting directly in the Scan Root. None of this is shown or adjustable.

### R3. "One linear pass: scan → review → run"
Review state lives only in memory on one screen. Cancel throws it away; changing a grouping setting means scanning again; there's no going back to a review once something ran.

### R4. "Safe means the app never finishes the job"
Nothing is ever deleted (ADR 0002), and the only Trash Destination is a Holding Folder *inside the folder being cleaned*. So the user can never actually get the disk space back through the app, even though the UI says "freed".

### R5. "The user reacts to suggestions; they don't author"
The user can accept, rename, skip or trim what the app suggests, but can't create a folder, merge two, act on the files the app didn't group, or write their own rules (Rules are frozen).

---

## Findings

### Duplicates

**D1. Exactly one Keeper; the Keeper itself is locked.** High · R1
`review-choices.js:43-51`, `screens/review.js:637`
The Keeper's Trash and Leave buttons are disabled. The only way to change it is to make another copy the Keeper, and the old Keeper is then forced to **Trash** (`review-choices.js:47`), not left as it was. A "Leave" option does exist for the other copies, so keeping two copies is *possible*, but:
- "Keep" and "Leave" both mean "this file stays", and the difference (only the Keeper goes into a Suggested Folder; Leave copies never do) isn't explained anywhere on screen.
- The user can't say "keep both of these and file both of them".
- Picking a new Keeper silently trashes the copy they may have meant to keep as well.
*User wants:* to mark each copy **Keep** or **Trash** independently, with the only rule being "at least one copy kept" (and even that could be a warning rather than a hard block, since everything is undoable).

**D2. Copies are shown by file name only.** High · R1
`screens/review.js:653`
Each copy row shows only its own name; the full path is in a hover tooltip. Duplicates very often share a name (`report.pdf` in two folders), so the user can't tell which copy is which, the one thing they need to know to choose. No folder, date or size is shown per copy either.
*User wants:* each copy's folder (relative to the Scan Root) and modified date visible in the row.

**D3. No way to open or preview a copy.** Medium
`file-list.js:57` (right-click only offers "Show in folder")
To check what a file actually is, the user has to leave the app.
*User wants:* Open / Quick Look from the row or the right-click menu.

**D4. No bulk decisions across groups.** High · R5
`screens/review.js:571-603`
With hundreds of groups the user decides one copy at a time. There's no "keep every copy that's in `Projects/`", "trash every copy in `Downloads/old/`", or "keep the oldest instead of the newest". No sort, filter or search over the groups either; 50 are shown at a time.
*User wants:* folder-based bulk rules ("prefer copies in…", "trash copies in…"), sort by size or by folder, and a filter.

**D5. Default Keeper is "newest", fixed.** Medium · R1
`core/src/duplicates.rs:206-222`
Newest-modified wins, then shortest path. Many people think of the *oldest* or the one in a particular folder as "the original". The rule isn't shown on screen and can't be changed.
*User wants:* a choice (newest / oldest / shortest path / in a preferred folder), shown as the reason a copy was picked.

**D6. After trashing, the Duplicates pane becomes a summary line.** Medium · R3
`screens/review.js:573-579`
Once the copies are trashed the groups disappear; the user can't see what was decided or reverse one copy from here. They have to go to History.
*User wants:* the groups to stay listed with their result, with per-copy Undo.

**D7. 0-byte files and Packages are never checked for duplicates.** Low · R2
`core/src/duplicates.rs:111`
Probably right as a default, but it's invisible.

### Scope of the scan

**S1. Hidden files, symlinks and files without an extension are dropped silently.** Medium · R2
`core/src/scanner.rs` (the walk loop: symlink, hidden and no-extension checks)
`ScanResult` counts `no_extension` and lists `unreadable`, but neither reaches the UI (no reference in `src/`). The user can't tell that `README`, `Makefile` or exported files with no extension were never looked at, or that some folders couldn't be read.
*User wants:* a line on the review screen ("142 files without an extension and 3 unreadable folders were left out") and a setting to include files with no extension.

**S2. One Scan Root at a time.** Medium · R1
`steps.js:16`, `GLOSSARY.md` ("Scan Root")
The user can't find duplicates *between* folders (e.g. Downloads vs Documents), which is one of the most common duplicate-hunting jobs.
*User wants:* several folders in one scan.

**S3. Never-Touch takes folders only.** Low
`screens/settings.js:133`
No single files, no patterns (`*.key`, `Archive*`).

### Suggested Folders

**G1. Only files sitting directly in the Scan Root are grouped.** High · R2
`grouping/src/lib.rs:239-249`
Files inside subfolders are only used as *samples* to describe that subfolder (5 per folder, `FOLDER_SAMPLE`). A messy `Downloads/` with stuff three levels deep only gets its top layer sorted, and nothing on screen says so.
*User wants:* to choose the depth ("just the top level" vs "everything"), or at least be told.

**G2. The user can't create a folder.** High · R5
`screens/review.js:214` (`destinations()` is only the suggested folders and "Left where they are")
There's no "New folder" in the sidebar, so if the app didn't suggest the folder they want, they're stuck.
*User wants:* "New folder", then drag files into it.

**G3. Can't merge, split or delete a Suggested Folder.** High · R5
`screens/review.js:722-844`
Two suggestions that should be one have to be emptied by hand, file by file or by drag. "Skip" is the only way to dismiss one, and its files then stay in the sidebar under a skipped folder rather than going back to "Left".
*User wants:* merge (drag a folder onto another), and remove (its files go back to Left).

**G4. Files in "Left where they are" can't be acted on.** High · R5
`screens/review.js:846-854` (`leftRow` has no actions), `screens/review.js:669` (not even selectable when there are no suggested folders)
The files the app *couldn't* sort are often the ones the user most wants to deal with, yet they can't be trashed from here, and when there are no suggestions at all they can't even be selected or dragged.
*User wants:* the same actions as any other file: select, trash, move into a folder (new or existing).

**G5. Files can only go into the suggested folders.** Medium · R1
`screens/review.js:214`
A file can be moved to another Suggested Folder or Left, but not straight into an existing folder on disk that the app didn't suggest. "Elsewhere…" exists only for a whole folder.
*User wants:* "Move to…" → pick any folder.

**G6. Changing grouping settings needs a full rescan.** Medium · R3
`screens/settings.js:163` ("Changes apply the next time files are grouped"); `grouping/src/lib.rs:587` (`Analysis::regroup` exists but nothing in `src/` or `src-tauri/src` calls it)
The backend can already regroup with new thresholds without running any model, but the UI doesn't offer it. The user has to go to Settings, come back Home and scan again to see if "looser" helps.
*User wants:* a "Fewer, bigger folders ↔ More, smaller folders" control *on the review screen* that regroups instantly.

**G7. Smallest folder is 2 files, minimum.** Low · R1
`screens/settings.js:233-256`
A single important file can't get its own folder suggestion; this matters less once G2 exists.

**G8. Archives, images and installers are never read and always get one folder per kind.** Low · R2
`grouping/src/lib.rs:7-10, 239-241`
Reasonable for speed, but all images in one "Images" folder may not be what the user wants (e.g. screenshots vs photos), and it isn't explained.

### Running and safety

**X1. Space is never freed, but the UI says "freed".** High · R4
`screens/review.js:598, 617` ("· N MB freed"); `screens/settings.js:110` (Holding Folder only); no code anywhere removes from the Holding Folder
Trashed files go to `Files Cleaner Trash` *inside the Scan Root*, on the same disk. Nothing in the app empties it, and the System Trash is deferred (ADR 0001). For a "cleaner", the user's main goal, getting space back, can't be finished in the app, and the "freed" number is not true.
*User wants:* an explicit, confirmed "Empty Files Cleaner Trash" (per Session or all), or the System Trash. This touches ADR 0002 ("never deletes permanently") and needs a deliberate decision, e.g. "never deletes *without the user asking*".

**X2. One blocked file blocks the whole folder run.** Medium · R1
`screens/review.js:248-254`
If any file can't move (e.g. "Elsewhere…" is on another disk), Move throws and nothing runs. The user is told to pick another place or skip the folder.
*User wants:* to run everything that can run, with the blocked files listed in the result.

**X3. Moves across disks are refused outright.** Medium · R4
`core/src/planner.rs` (`across_disks`), ADR 0002 Consequences
Sensible for the Journal, but it means "Elsewhere…" on an external drive just fails.
*User wants:* copy-then-trash-the-original (still undoable), or at least to know beforehand in the folder picker.

**X4. Cancel discards the whole review.** Medium · R3
`screens/review.js:220-227`
Every Keeper choice, rename and drag is lost, with no confirmation. The same happens when the app is closed mid-review.
*User wants:* a confirmation, and ideally the review saved so it can be resumed.

**X5. "Needs attention" items have no action.** Medium
`screens/session.js:21` ("Needs attention has no action")
Skipped or failed Operations can't be retried from the Session screen; the user can only look at them.
*User wants:* "Try again" for skipped/failed items, and "Show in folder" at least.

**X6. Sessions can't be deleted or cleared from History.** Low
`screens/history.js:29`

### Settings and control

**C1. The user's own Rules are switched off.** Medium · R5
`README.md` ("Rules … are frozen"), `core/src/planner.rs` (`build_rule_plan`, unused)
The one feature that let the user *tell* the app what they want (".dmg → Trash", "PDFs from my bank → Finance/") is disabled. Together with D4 and G2, it leaves the user only able to react.
*User wants:* Rules back, probably as "apply to these files" actions on the review screen rather than a separate step.

**C2. Recent Scan Roots capped at three.** Low
`GLOSSARY.md` ("Recent Scan Root")

---

## Suggested order

1. **Re-model duplicates (D1, D2, D5, D4).** Replace "the Keeper" with a per-copy Keep/Trash choice (at least one kept), show each copy's folder and date, add folder-based bulk choices. This is the change you named, and it's self-contained: `review-choices.js`, `duplicateCopy` in `review.js`, `DuplicateChoice` in `suggested_folders.rs`, and `GLOSSARY.md`.
2. **Let the user author folders (G2, G3, G4, G5).** New folder, merge, remove, and full actions on "Left where they are". This is the R5 fix and mostly UI on top of the existing `choices` model.
3. **Make the job finishable (X1).** Decide on emptying the Holding Folder or the System Trash, and record it as an ADR that amends 0002. Remove "freed" until then.
4. **Show the scope (S1, G1, G8).** One honest line on the review screen about what was left out, then settings for depth and files without an extension.
5. **Regroup in place (G6).** Wire `Analysis::regroup` to a control on the review screen.
6. **Robustness (X2, X4, X5, D6).** Partial runs, confirm on Cancel, retry from the Session, keep the duplicate results visible.
7. **Bring back Rules as user actions (C1)**, once 1 and 2 have settled what a "choice" is.

Steps 1, 2 and 3 each change a GLOSSARY term or an ADR, so they're worth grilling and recording with `domain-modeling` before any code changes.

## Comments
- 2026-10-29: Steps 1–2 grilled and settled. Decisions: `GLOSSARY.md` (Kept Copy, Filed Copy, Default Copy Rule, Suggested Folder), ADR 0007; tickets in `issues/01`–`05`. Step 3 (X1, emptying Files Cleaner Trash) deferred to later. Nothing implemented yet.
- 2026-10-29: Every preference lives on the Settings screen only, never as a control on Review or any other screen. So step 5 (G6's in-place regroup control) is dropped, and D5's rule is a Settings option (ticket 02). G7 (smallest folder) stays a Settings option as it is.
