# 06: Clearer Suggested Folders and reading progress

**What to build:** On today's Suggested Folders step, the user can see what they can do and how long the wait is.
- A Suggested Folder can be renamed in the sidebar: double-click it, or press Return when it's selected. Return or clicking away saves, Esc cancels, and an empty name is refused ("A folder needs a name."). The name field in the pane and the sidebar always show the same name.
- A folder's choices (Create here, Add to X when matched, Elsewhere…, Trash all, Skip) are a segmented control instead of a dropdown, with the line saying where the folder goes beside it.
- Trash, Take out and Put back on each file are small bordered buttons, always visible.
- Reading shows "0 of N files" at once. N counts only files that need reading: files with saved readings are left out, and the sampled subfolder files are counted in. "Loading the grouping models…" shows before the first file.
- Beside the counter: "About X s per file on this Mac · about M min left". The figure comes from the average stored for this computer, or 4 s when there's no history. After about 5 files it blends in this run's pace. It covers everything from starting to read until the folders are shown, including model loading and the "Finding folders for files left out" pass.
- When a run finishes, its total time divided by the files read updates the stored average (in the app's data, not the config file). A cancelled run doesn't update it.
- The text "Reading takes a while. You can switch to other apps; Files Cleaner will notify you when the folders are ready." is shown while reading.
- A system notification says "Folders are ready for {Scan Root name}", or "Couldn't finish reading {Scan Root name}" on error, only while the app window isn't focused. Permission is asked when reading first starts. A cancel sends nothing.

See `../one-screen-review.md`: user stories 13–17, 33–35, 37–52.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] Sidebar inline rename (double-click / Return, Esc cancels, empty refused), in sync with the pane's name field
- [x] Segmented control replaces the folder choice dropdown, same choices and the same "Elsewhere…" picker
- [x] Trash / Take out / Put back are small bordered buttons, always visible
- [x] Progress reports a model-loading phase and the total up front, counting only files that need reading
- [x] Reading estimate (pure module): stored average or 4 s, blended with this run's pace after ~5 files; the average is updated when a run finishes and stored per computer in app data
- [x] Wait text, and success / failure notifications through Tauri's notification plugin, only when the app is in the background; nothing on cancel
- [x] Quiet native styling; no "AI" wording
- [x] Test: with no history, the time left is 4 s × the files left; a finished run updates the stored average

## Comments

- 2026-10-28: Done. On the desktop, Tauri's notification plugin always reports permission as granted and never asks, so a refusal can't be detected. Instead, the reading screen asks once ("Notify you when the folders are ready?" with Notify me / Don't notify). The answer is kept in the config as `"notifications": true | false`, and the question isn't asked again. After a refusal, nothing is sent and the wait text leaves out the notification. Reading also shows a "Checking which files were read before" step before "0 of N", because counting only unread files means checking each file first.
