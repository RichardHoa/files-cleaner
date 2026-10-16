# 04: Emptied folders, cross-disk moves, and crash recovery

**What to build:** The safety edges. Folders the Plan would empty are offered as unticked "trash empty folder" items. Moves to another disk are shown as errors that can't be ticked. A Session that crashed halfway through can still be undone, and older Sessions can be undone safely even after newer ones touched the same files.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] The Planner predicts folders that become empty and adds Trash Operations for them, unticked by default
- [ ] At run time an emptied folder is trashed only if it is truly empty (a folder holding only OS Metadata Files, such as `.DS_Store` or `Thumbs.db`, counts as empty); otherwise it's skipped with a reason
- [ ] Moves, and Trash to a Holding Folder, that cross a volume boundary appear in the Plan as errors that can't be ticked and are never executed
- [x] Undo of a crashed Session: Operations journaled as intent but never finished are resolved by checking the disk, then undone or skipped accordingly (tested with hand-written Journals and matching disk state)
- [x] The Session list shows crashed Sessions as such, and they can be undone
- [ ] Undoing an older Session after a newer Session touched the same files skips and reports the conflicting items and undoes the rest
