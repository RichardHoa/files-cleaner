# 04: New folder, merge, remove; no Skip

**What to build:** The user shapes the Suggested Folders.
- **New folder** in the sidebar adds "untitled folder", in rename mode with its name selected, created in the Scan Root by default.
- **Merge**: drag a folder onto another, or "Merge into…" in its right-click menu; the folder dropped onto keeps its name.
- **Remove** in the folder's right-click menu sends its files to Left where they are.
- Skip is gone from the folder's choices.

**Blocked by:** none

**Status:** resolved

- [x] New folder, Merge, Remove
- [x] Skip removed from UI and backend
- [x] Test: merging moves every file into the target and drops the merged folder

## Comments
- 2026-11-05: Done, not yet run in the app. `addFolder`, `mergeFolder` and `removeFolder` in `review-choices.js`; the button, the drag and `folderMenu` in `screens/review.js`; `FolderChoice::Skip` removed from `suggested_folders.rs`. A Duplicate Group that remembers its place in a dropped folder now remembers the merge target, or Left. Behaviour the ticket didn't spell out: the target keeps where it goes as well as its name, so merging into a "Trash all" folder trashes the incoming files, and merging out of one no longer trashes them; a file's own Trash or Take out mark survives a merge, but not a Remove; a new folder left empty is never created, like any folder with no files. Check by hand: dragging a sidebar folder in the webview (the first draggable `<button>` here), and that the name stays selected through the sidebar redraw about 300 ms after New folder. Open question for the developer: should merging into "Trash all" warn?
