# 04: Plan safety: emptied folders, blocked moves, and crash recovery

**What to build:** The safety edges. Folders the Plan would empty are offered as unticked "trash empty folder" items. Moves that can't safely happen (to another disk, or into a Protected Path or Package) are shown as errors that can't be ticked, instead of being dropped without a word. A Session that crashed halfway through can still be undone, and older Sessions can be undone safely even after newer ones touched the same files.

This is also the prerequisite for releasing content grouping (`.scratch/content-grouping/`), whose Plans move many files out of subfolders and into existing folders elsewhere on disk.

**Blocked by:** None (03 is resolved)

**Status:** ready-for-agent

- [ ] The Planner predicts folders that become empty and adds Trash Operations for them, unticked by default
- [ ] At run time an emptied folder is trashed only if it is truly empty (a folder holding only OS Metadata Files, such as `.DS_Store` or `Thumbs.db`, counts as empty); otherwise it's skipped with a reason
- [ ] Moves, and Trash to a Holding Folder, that cross a volume boundary appear in the Plan as errors that can't be ticked and are never executed
- [ ] A Rule whose relative `moveTo` resolves to a Protected Path or Package below the Scan Root (e.g. `Files Cleaner Trash/x`, `proj/node_modules`) shows its matching files in the Plan as errors that can't be ticked, saying why, instead of proposing nothing (moved here from ticket 03)
- [x] Undo of a crashed Session: Operations journaled as intent but never finished are resolved by checking the disk, then undone or skipped accordingly (tested with hand-written Journals and matching disk state)
- [x] The Session list shows crashed Sessions as such, and they can be undone
- [ ] Undoing an older Session after a newer Session touched the same files skips and reports the conflicting items and undoes the rest
