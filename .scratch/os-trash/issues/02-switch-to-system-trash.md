# 02: Switch Trash to the System Trash

**What to build:** Every Trash goes to the System Trash on all three OSes, and Undo and Redo bring it back from where it landed. The `Files Cleaner Trash` folder, its code and the Settings section are removed.
- A file that can't be trashed stays in place and is listed under Needs attention, while the rest of the Plan runs. It's shown as blocked in the Plan when that can be told up front.
- Emptied from the Trash: Unrecoverable, with a Note, and the rest of the Session stays undoable.
- Crash before the location was recorded: "may be in the Trash, restore it by hand", with no searching.
- Screens say "to Trash" or "to Recycle Bin", and never "freed".

See `../spec.md`: all of *Implementation Decisions* except Delete all Sessions; user stories 1–27 and 35–37.

**Blocked by:** 01

**Status:** ready-for-agent

- [x] OS Trash interface in `core`, with a backend per OS shaped by ticket 01's results
- [x] Journal records each Trash's location after it happens; old Journals still read and undo
- [ ] Run, Undo and Redo use the interface; no code path can permanently delete (unproven on Windows, see below)
- [x] Holding Folder code and scanner check removed; a `trashDestination` key in an old config is ignored
- [x] Trash section gone from Settings; per-OS wording on Review and the Session screen
- [x] If ticket 01 found macOS needs it: Full Disk Access is requested only when Undo needs it, with the Put Back note if not granted
- [x] README updated
- [x] Test: with a fake OS Trash, trash then Undo puts the file back; removing it from the fake first gives Unrecoverable, and the rest of the Session stays undoable
- [x] Test: when the fake refuses a file, it stays in place, is Skipped, and the rest of the Plan runs
- [x] Test: an old-format Journal still undoes from its recorded destination

## Comments

**Agent (2026-11-05):** Built on all three OSes, uncommitted, with ticket 01 still open. Automated tests use a fake; no real Trash has been touched by this code, and the app hasn't been run.

- **Checked:** core and JS tests pass on macOS. The Linux backend passes `cargo check` and the Windows backend passes clippy from macOS (with `--features blake3/pure`), neither linked or run.
- **Before release:** ticket 01's Windows and Linux runs are still needed. Windows is the risk, as the only check is that the sink refuses any delete without `TSF_DELETE_RECYCLE_IF_POSSIBLE`, documented only as "if possible". If ordinary recycles arrive without the flag, every Trash is refused and left in place (safe, but it won't work). If the flag is also set on a delete the Bin can't take, that delete goes through for good, against ADR 0001. The run has to settle this.
- **Not built:** Review doesn't show a Trash as blocked before the Run (story 12); it counts it as going to the Trash, then the Session lists it under Needs attention. An Emptied Folder blocked this way says "Can't move". Only a drive with no Trash reaches this, on Windows and Linux.
- **Undo checks what it restores:** a name in the Trash can be reused once it's emptied, so an item whose size or modified date no longer matches counts as gone, and stays where it is.
- **Folder Undo:** an Emptied Folder whose files were put back first merges with `merge_folder`, not `put_back`, so it misses the Full Disk Access Note and the check above.
- **Windows `$I` records:** Undo moves the `$R` item out and leaves its `$I` record, as ticket 01 hasn't said whether that matters. Its run prints the leftover.
- **macOS, blocked in the Plan:** never blocks up front, as no-Trash detection is untested and could wrongly block a disk that would make a Trash. Such a file is Skipped at Run instead.
- **Full Disk Access:** per the developer's decision on ticket 01, a refused Undo's Note says to turn it on in System Settings or use Put Back in Finder. The app doesn't open the pane. `~/.Trash`'s need for it is still untested.
- **Choices the ticket didn't spell out:** a Trash blocked in the Plan is approved by Review and journaled as Skipped, so it's listed under Needs attention. "Show in folder" is hidden for items in the System Trash. An older Session's Trash reads like a Move (named by its folder). Redo of an older Session's Trash goes to the System Trash.
- **Left as is:** putting back checks the destination then renames, like every other move in `core`, so an item appearing in that gap could be overwritten. Ticket 01 flagged this as optional.
