# 04: Plan safety: emptied folders, blocked moves, and crash recovery

**What to build:** The safety edges. Folders the Plan would empty are offered as unticked "trash empty folder" items. Moves that can't safely happen (to another disk, or into a Protected Path or Package) are shown as errors that can't be ticked, instead of being dropped without a word. A Session that crashed halfway through can still be undone, and older Sessions can be undone safely even after newer ones touched the same files.

This is also the prerequisite for releasing Suggested Folders (`.scratch/content-grouping/`), whose Plans move many files into new and existing folders, often right after a duplicate Session touched the same folder.

**Blocked by:** None (03 is resolved)

**Status:** ready-for-agent

- [x] The Planner predicts folders that become empty and adds Trash Operations for them, unticked by default
- [x] At run time an emptied folder is trashed only if it is truly empty (a folder holding only OS Metadata Files, such as `.DS_Store` or `Thumbs.db`, counts as empty); otherwise it's skipped with a reason
- [x] Moves, and Trash to a Holding Folder, that cross a volume boundary appear in the Plan as errors that can't be ticked and are never executed
- [x] Undo of a crashed Session: Operations journaled as intent but never finished are resolved by checking the disk, then undone or skipped accordingly (tested with hand-written Journals and matching disk state)
- [x] The Session list shows crashed Sessions as such, and they can be undone
- [x] Undoing an older Session after a newer Session touched the same files skips and reports the conflicting items and undoes the rest

## Comments

**2026-10-20.** Dropped the item about a Rule's relative `moveTo` resolving to a Protected Path: Rules are frozen (`.scratch/basic-pipeline/map.md`). The same guard for a Suggested Folder's destination belongs to the Suggested Folders spec. Moves to another disk stay blocked for the basic goal; copying across disks comes with cloud drives.

**2026-10-20, implemented.**
- Emptied folders: `build_plan` offers only the topmost folder that every item leaves (OS Metadata Files and folders that are empty too don't count; hidden files, protected items, files with no extension and anything a Move puts there do), as a Trash with Reason `EmptiedFolder`, `ticked: false`, run last. `SourceState.isFolder` marks it in the Journal, so Run and Redo check "still empty" instead of size and date. Its files usually went to the same place in the Holding Folder first, so the folder gets a Clash Suffix there (`old (1)`). If its files were undone on their own (which makes the folder again), undoing the folder merges what's left (its `.DS_Store`, empty subfolders) back without overwriting.
- Blocked moves: each Operation gets `error` (and is unticked) when its source and destination are on different disks (device ID on macOS/Linux, drive on Windows). `run` never runs one, even if approved, and doesn't journal it. The Plan screen shows "Can't move" with a disabled tick box and the reason.
- Older Sessions: Undo reads the Sessions started after this one. An item a newer Session has moved on, or a place a newer Session has filled, is skipped with "Session <id> has since …; undo that Session first", not reported as unrecoverable. A Create Folder is now kept while items this Session moved into it aren't back yet, so it isn't left behind when they come back later.
- Tests: `core/tests/safety.rs` (9; the cross-disk one mounts a tmpfs and needs root). End-to-end on a generated 5,000-file, 3.2 GB tree: 252 Duplicate Groups matched an independent hash, run + undo byte-for-byte, `kill -9` mid-run then undo exact, 45 real older/newer conflicts, and a mounted second disk.
