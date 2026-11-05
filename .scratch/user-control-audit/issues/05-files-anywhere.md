# 05: Act on any file, move it anywhere

**What to build:**
- Files under Left where they are are always selectable and draggable, and can be trashed, also when no folders were suggested.
- "Move to…" offers "Other folder…", which picks any folder on disk; the file moves there.

**Blocked by:** 04

**Status:** resolved

- [x] Left files selectable and trashable in every case
- [x] Other folder… moves the selection into a picked folder
- [x] Test: a file moved to another folder ends up there in the Plan

## Comments
- 2026-11-05: Done, not yet run in the app. `LeftFiles` and `moveToOtherFolder` in `review-choices.js`; the Trash / Put back button, "Other folder…" in the Move to… menu and "Move to other folder…" in the right-click menu in `screens/review.js`. Behaviour the ticket didn't spell out: Other folder… adds a Suggested Folder going into the picked folder (or reuses the one already going there), so it shows in the sidebar and can be changed or removed like any other; a file already directly in the picked folder is left where it is, and `build_review_plan` now also leaves such a file alone, where before it got a Clash Suffix; a Kept Copy that isn't filed still can't be selected or trashed from Left, only from Duplicates, since it stays where it is by definition; files trashed from Left are planned as a Trash-all folder named "Left where they are", so their Reason is a Suggested Folder of that name. Trash is per row, like in a folder's pane, not on a selection. No check that the picked folder isn't a Protected Path, the same as "Elsewhere…". A folder on another disk is refused as soon as it's picked, with a message; "Elsewhere…" still only fails at Move. The header and button now say what will happen ("1 file will be trashed", "Move 2, trash 1"), and the count leaves out a file already in the folder it goes to. Check by hand: the Left row's date and button layout, that the Move to… menu resets after cancelling the folder picker, and picking a folder on another disk. Open question for the developer: should Trash also act on a selection, here and in folders?
