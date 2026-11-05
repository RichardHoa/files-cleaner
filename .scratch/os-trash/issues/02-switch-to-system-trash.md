# 02: Switch Trash to the System Trash

**What to build:** Every Trash goes to the System Trash on all three OSes, and Undo and Redo bring it back from where it landed. The `Files Cleaner Trash` folder, its code and the Settings section are removed.
- A file that can't be trashed stays in place and is listed under Needs attention, while the rest of the Plan runs. It's shown as blocked in the Plan when that can be told up front.
- Emptied from the Trash: Unrecoverable, with a Note, and the rest of the Session stays undoable.
- Crash before the location was recorded: "may be in the Trash, restore it by hand", with no searching.
- Screens say "to Trash" or "to Recycle Bin", and never "freed".

See `../spec.md`: all of *Implementation Decisions* except Delete all Sessions; user stories 1–27 and 35–37.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] OS Trash interface in `core`, with a backend per OS shaped by ticket 01's results
- [ ] Journal records each Trash's location after it happens; old Journals still read and undo
- [ ] Run, Undo and Redo use the interface; no code path can permanently delete
- [ ] Holding Folder code and scanner check removed; a `trashDestination` key in an old config is ignored
- [ ] Trash section gone from Settings; per-OS wording on Review and the Session screen
- [ ] If ticket 01 found macOS needs it: Full Disk Access is requested only when Undo needs it, with the Put Back note if not granted
- [ ] README updated
- [ ] Test: with a fake OS Trash, trash then Undo puts the file back; removing it from the fake first gives Unrecoverable, and the rest of the Session stays undoable
- [ ] Test: when the fake refuses a file, it stays in place, is Skipped, and the rest of the Plan runs
- [ ] Test: an old-format Journal still undoes from its recorded destination

## Comments
