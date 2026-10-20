# 04: Plan safety: emptied folders, blocked moves, and crash recovery

**What to build:** The safety edges. Folders the Plan would empty are offered as unticked "trash empty folder" items. Moves that can't safely happen (to another disk, or into a Protected Path or Package) are shown as errors that can't be ticked, instead of being dropped without a word. A Session that crashed halfway through can still be undone, and older Sessions can be undone safely even after newer ones touched the same files.

This is also the prerequisite for releasing Suggested Folders (`.scratch/content-grouping/`), whose Plans move many files into new and existing folders, often right after a duplicate Session touched the same folder.

**Blocked by:** None (03 is resolved)

**Status:** ready-for-agent

- [ ] The Planner predicts folders that become empty and adds Trash Operations for them, unticked by default
- [ ] At run time an emptied folder is trashed only if it is truly empty (a folder holding only OS Metadata Files, such as `.DS_Store` or `Thumbs.db`, counts as empty); otherwise it's skipped with a reason
- [ ] Moves, and Trash to a Holding Folder, that cross a volume boundary appear in the Plan as errors that can't be ticked and are never executed
- [x] Undo of a crashed Session: Operations journaled as intent but never finished are resolved by checking the disk, then undone or skipped accordingly (tested with hand-written Journals and matching disk state)
- [x] The Session list shows crashed Sessions as such, and they can be undone
- [ ] Undoing an older Session after a newer Session touched the same files skips and reports the conflicting items and undoes the rest

## Comments

**2026-10-20.** Dropped the item about a Rule's relative `moveTo` resolving to a Protected Path: Rules are frozen (`.scratch/basic-pipeline/map.md`). The same guard for a Suggested Folder's destination belongs to the Suggested Folders spec. Moves to another disk stay blocked for the basic goal; copying across disks comes with cloud drives.
