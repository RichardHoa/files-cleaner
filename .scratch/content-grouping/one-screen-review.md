# One screen to review duplicates and Suggested Folders

The spec behind tickets `issues/06`–`08`; builds on ticket 04. See `docs/adr/0005-one-plan-for-duplicates-and-suggested-folders.md`. The grouping settings and model details in Settings are ticket 05.

## Problem Statement

Cleaning a folder takes too many screens and too much waiting, and the screens don't make it clear what will happen:

- After the scan, the user lands on a Scan results table that only leads to "Find duplicates".
- The duplicate screen lists every copy as a loose row, with a separate pane on the right. That pane repeats the file name and explains the move in a sentence, and the file that stays appears only as an "Original:" path. The user can't see at a glance which copy is kept.
- After the duplicate Session, the app rescans and then reads every file. That can take many minutes, and the user only sees "Reading the files…" at first. There's no sense of how long it will take, and no way to go do something else and be told when it's done.
- On the Suggested Folders screen, what can be done to a folder (create elsewhere, trash all, skip) is hidden in a dropdown, so it's easy to miss. Trash and Take out look like text, not buttons. A folder can only be renamed in the pane, not where the folders are listed.

## Solution

After the scan, the user goes straight to one review screen that holds the whole Plan: duplicates, Rule matches, Suggested Folders, and the files left where they are. It is approved once and carried out as one Session.

- The Duplicates section shows each Duplicate Group as a unit. Every copy has its folder, the Keeper is marked **Keep**, and each other copy is **Trash** or **Leave**. Any copy can be made the Keeper.
- Duplicates show as soon as the scan ends. The Suggested Folders are filled in while the files are read in the background. The reading shows a counter from the start, an estimated time left (based on how long a file took on this computer before), and a note that the user can switch apps and will be notified when the folders are ready.
- Each Suggested Folder shows its choices as a segmented control. Its files have real Trash / Take out / Put back buttons. It can be renamed by double-clicking it in the sidebar.

## User Stories

### Flow

1. As a user, I want the review screen to open right after the scan, so that I don't pass through a table that only leads to the next step.
2. As a user, I want a one-line summary of the scan (items and total size) at the top of the review screen, so that I still know what was scanned.
3. As a user, I want duplicates, Rule matches and Suggested Folders on one screen, so that I clean the folder in one place.
4. As a user, I want one Approve for the whole Plan, so that I don't approve twice.
5. As a user, I want the whole clean-up to be one Session in History, so that I can undo it in one go.
6. As a user, I want to undo only some Operations of that Session, so that I can bring back just the duplicates or just the folder moves.
7. As a user, I want no rescan between duplicates and folders, so that I don't wait twice.
8. As a user, I want Cancel to stop any reading, discard the Plan and go Home with nothing changed, so that backing out is safe.
9. As a user, I want what was already read to be kept when I cancel, so that the next time is faster.
10. As a user without the Grouping Models, I want the screen to show the Duplicates and Rule sections plus the models offer, so that I can still remove duplicates.

### Sidebar

11. As a user, I want the sidebar to list Duplicates first, then each Rule, then Left where they are, then each Suggested Folder, then Emptied Folders, so that I can step through the Plan in order.
12. As a user, I want each sidebar entry to show how many items it has, so that I see the size of each part.
13. As a user, I want to rename a Suggested Folder by double-clicking it in the sidebar, so that I can fix names where I see them.
14. As a user, I want Return on a selected Suggested Folder to start renaming it, as in Finder, so that I can rename from the keyboard.
15. As a user, I want Return or clicking away to save the name and Esc to cancel, so that renaming behaves as I expect.
16. As a user, I want an empty name to be refused, so that a folder never ends up without a name.
17. As a user, I want the pane's name field and the sidebar to always show the same name, so that I'm never unsure which name counts.

### Duplicates

18. As a user, I want each Duplicate Group shown as one entry with its file name, number of copies and space freed, so that I can judge it at a glance.
19. As a user, I want every copy in a group listed with its full folder, so that I can see where each copy lives.
20. As a user, I want the Keeper listed first and marked **Keep**, so that it's obvious which file stays.
21. As a user, I want each other copy marked **Trash** by default, so that the usual case needs no clicks.
22. As a user, I want to set a copy to **Leave**, so that I can keep two copies on purpose.
23. As a user, I want a copy I leave to stay exactly where it is, outside any Suggested Folder, so that leaving it means leaving it alone.
24. As a user, I want to make any copy the Keeper, so that I keep the copy in the place I prefer, not just the newest.
25. As a user, I want the old Keeper to be trashed when I pick a new one, so that the group still ends with one copy.
26. As a user, I want a new Keeper to take the old Keeper's place in its Suggested Folder (or in Left where they are), so that changing the Keeper doesn't need the files read again.
27. As a user, I want the Duplicates entry's count and the header's "N files will move" to update as I change copies, so that I always know the total.
28. As a user, I want no separate explanation pane for duplicates, so that the list itself says everything.
29. As a user, I want to show any copy in Finder from its right-click menu, so that I can check it before deciding.

### Rules and Emptied Folders

30. As a user, I want each Rule's matches under its own sidebar entry, so that I can review what each Rule does.
31. As a user, I want files matched by a Rule left out of the Suggested Folders, so that a Rule's move is never contradicted.
32. As a user, I want Emptied Folders worked out from the whole Plan, folder moves included, and unticked by default, so that only truly empty folders are offered.

### Suggested Folders

33. As a user, I want a folder's choices (Create here, Add to X when it matches, Elsewhere…, Trash all, Skip) shown side by side as a segmented control, so that I see everything I can do without opening a menu.
34. As a user, I want the line describing where the folder goes kept beside the control, so that I know what the current choice means.
35. As a user, I want Trash, Take out and Put back on each file to be real buttons, always visible, so that I know they can be clicked.
36. As a user, I want copies marked for Trash and Rule matches never read for grouping, so that reading takes less time.

### Reading

37. As a user, I want to see "0 of N files" as soon as reading starts, so that I know how much there is.
38. As a user, I want "Loading the grouping models…" shown while the models load, so that I know why the first file takes a while.
39. As a user, I want "Finding folders for files left out" shown during the last pass, so that I know it is still working.
40. As a user, I want to be told that reading takes a while and that I can switch to other apps and will be notified when it's done, so that I don't sit and watch.
41. As a user, I want a system notification when the folders are ready and the app is in the background, so that I come back at the right time.
42. As a user, I want no notification when the app is in front, so that I'm not interrupted for something I can see.
43. As a user, I want to be asked for notification permission the first time reading starts, so that the request comes with a clear reason.
44. As a user, I want a notification if reading fails while the app is in the background, so that I don't wait for something that won't come.
45. As a user, I want no notification when I stop or cancel reading myself, so that I'm only told about what I didn't do.
46. As a user, I want to see how long a file takes to read on this computer ("about 4 s per file on this Mac"), so that I understand the wait.
47. As a user, I want a countdown of the time left, so that I can plan around it.
48. As a first-time user, I want the estimate to start at about 4 s per file, so that I get a figure even with no history.
49. As a user, I want the countdown to adjust to this run's own pace after a few files, so that it stays close to the truth.
50. As a user, I want files that were already read to count as no time, so that the countdown isn't inflated.
51. As a user, I want the estimate to cover everything from starting to read to seeing the folders, including model loading and the last pass, so that the countdown ends when the folders appear.
52. As a user, I want the per-file time remembered across runs, so that the estimate improves with use.
53. As a user, I want Approve to wait until reading finishes and say so ("Waiting for folders…"), so that I don't approve half a Plan by accident.
54. As a user, I want a **Stop reading** button, so that I can stop early when I only care about the duplicates.
55. As a user, I want files read before stopping to be grouped and the rest left where they are, so that stopping keeps the work already done.
56. As a user, I want to work through duplicates while reading goes on, so that the wait isn't wasted.

## Implementation Decisions

- **Flow.** The step flow becomes pick folder → scan (with progress) → review. The Scan results screen, the duplicate Plan screen, the duplicate Session's "Continue" and the rescan are removed. The step flow is still the one module that owns the order, given its commands so it can be driven with fakes.
- **One Plan.** The backend builds one Plan for the Scan Root from: Duplicate Groups with each copy's choice (Keeper / Trash / Leave), Rule Operations, the Suggested Folder choices, and the Emptied Folders that result from all of them. The Reasons stay as they are (Duplicate naming its Keeper, Rule, Suggested Folder, Emptied Folder). Approving runs the ticked Operations as one Session with one Journal, which reuses the existing run command.
- **Duplicates on the backend.** The duplicate finder reports Duplicate Groups (every copy's path, size, dates and the default Keeper), not only per-copy Operations, so the UI can show groups and change the Keeper. The Keeper default rule is unchanged.
- **What reading covers.** Reading starts as soon as the scan ends, on every loose file except copies whose default choice is Trash and files matched by a Rule. Keepers are read. Grouping no longer gets a fresh scan, per ADR 0005. A copy the user later sets to Leave stays where it is and is never added to a folder. A copy promoted to Keeper inherits the old Keeper's place. There's no new reading, because the contents are identical.
- **Review state.** The pure review-choices module grows to hold the Duplicate Groups' choices alongside the Suggested Folder choices: set copy to Keeper, Trash or Leave; rename; move files; the folder choice. It produces the payload for building the one Plan, plus the header totals. The screen only draws this state.
- **Stop reading.** Stopping asks the analysis to stop between files, as cancel does now, but then groups the cards read so far and puts every unread file in Left where they are. Cancel stops the analysis too, but goes Home and builds nothing.
- **Progress.** The analysis progress event reports a model-loading phase before the first file, and the total is sent at once, so "0 of N" shows immediately. The total counts only files that actually need reading, leaving out the ones with saved readings. The sampled subfolder files are included, since they are read.
- **Reading estimate.** A small pure module turns `{ stored average seconds per file, files left, this run's elapsed time and files done }` into the per-file figure and the time left. Before about 5 files it uses the stored average (4 s when there is none). After that it blends in this run's pace. When a run finishes (not when it's stopped or cancelled), the run's total time from reading start until the folders are shown is divided by the files read, and the stored average is updated. The average is stored per computer in the app's data, not in the config file.
- **Notifications.** Add Tauri's notification plugin. Permission is asked when reading first starts. "Folders are ready for {Scan Root name}" is sent on success, and "Couldn't finish reading {Scan Root name}" on error, each only while the app window isn't focused. Stop and cancel send nothing.
- **Wait text.** "Reading takes a while. You can switch to other apps; Files Cleaner will notify you when the folders are ready." It's shown with the counter, the per-file time and the countdown, where the Suggested Folders will appear.
- **Sidebar rename.** Double-click or Return on a selected Suggested Folder turns its label into an inline text field. Return or blur saves, Esc reverts, and an empty name is refused with the existing "A folder needs a name." The pane's name field edits the same choice and both redraw from it.
- **Folder choice control.** A native-looking segmented control replaces the `select`, with the same choices and the same "Elsewhere…" folder picker.
- **File buttons.** Trash / Take out / Put back become small bordered buttons, always visible, in the existing quiet button style.
- **Wording.** Use Keeper wording ("Keep"), never "Original". No "AI" wording, and quiet native styling.

## Testing Decisions

- Keep to one or two best-case tests per piece for now. Test what a caller sees, not how it's built.
- **Step flow:** with fake commands, picking a folder scans and then shows the review, with no Scan results screen and no second scan. Prior art: the existing step flow tests with fakes. Those tests are replaced.
- **Review choices:** making another copy the Keeper puts it in the old Keeper's Suggested Folder and marks the old Keeper for Trash. A copy set to Leave is in no folder and isn't trashed. Prior art: the existing Suggested Folder choices tests.
- **Reading estimate:** with no history, the time left is 4 s × the files left. A finished run updates the stored average. This is a new pure module with plain unit tests, like the formatting tests.
- No tests for the notification, sidebar rename, segmented control or buttons. The developer checks the UI themselves.

## Out of Scope

- The grouping thresholds, "Read every file again" and the Grouping Models' details in Settings (ticket 05).
- Live regrouping on the review screen. There is no slider there any more.
- Sorting by kind, name and date without the Grouping Models.
- The System Trash as a Trash Destination.
- Windows and Linux checks for the notification beyond building. The plugin covers them, but they're checked when the ports are.

## Further Notes

- This replaces `pipeline/02`'s step order and its "grouping never uses the scan from before the duplicate Session" rule (ADR 0005).
- The "N files will move" header counts trashed files too, as both steps do now.
