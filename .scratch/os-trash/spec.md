# OS Trash: Trash goes to the System Trash, and stays undoable

Status: ready-for-agent

## Problem Statement

I use Files Cleaner to get my disk space back, but today it can't finish that job. Trashed files move to a `Files Cleaner Trash` folder inside the folder I'm cleaning, on the same disk, and nothing in the app ever empties it. The Review screen still says "N MB freed", which isn't true: not one byte has been freed.

The folder is also clutter of the app's own making, in the very place I asked it to tidy, and it's a different kind of "trash" from the one I already know. To really get rid of the files I have to find that folder and delete it by hand.

## Solution

Trash has one behaviour, with no setting: it sends files to the System Trash (the Trash on macOS and Linux, the Recycle Bin on Windows). I empty it the way I always have, and that's when I get the space back. The app never deletes a file permanently.

Undo and Redo keep working: the app records where the OS put each file, and can put it back until I empty it from the Trash. After that, the Session stays in History, with those Operations marked as no longer recoverable.

If a file can't go to the System Trash (say, on a network drive), it stays where it is and is listed under Needs attention, and the rest of the Plan still runs. A trash never turns into a permanent delete.

The `Files Cleaner Trash` folder and all the code for it go away. Settings loses its Trash section and gains **Delete all Sessions**, which clears History and the Journals but no files.

## User Stories

### Trashing

1. As a user, I want trashed files to go to my OS's own Trash, so that I get rid of them the way I already know.
2. As a user, I want no setting for where Trash goes, so that there's nothing to decide or get wrong.
3. As a user, I want no extra folder left in the folder I'm cleaning, so that the app doesn't add clutter while tidying.
4. As a user, I want the space to come back when I empty the Trash, so that the app's job can really be finished.
5. As a user, I want a whole trashed folder or Package to go to the Trash as one item, so that it comes back as one item.
6. As a user, I want two trashed files with the same name to both be safe in the Trash, so that neither overwrites the other.
7. As a user, I want the app to never empty the Trash or clean up by itself, so that nothing disappears without my asking.
8. As a user, I want no "Empty" button in the app, so that permanent deletion stays a separate, deliberate act in my OS.

### Files that can't be trashed

9. As a user on a drive with no Trash, I want the file left where it is, so that a trash never becomes a permanent delete.
10. As a user, I want such a file listed under Needs attention with a Note saying why, so that I know it's still there and what to do.
11. As a user, I want the rest of the Plan to run when one file can't be trashed, so that one file doesn't block all my work.
12. As a user, I want to see before the Run that a file can't be trashed, when the app can tell, so that the result doesn't surprise me.

### Undo and Redo

13. As a user, I want Undo to put a trashed file back where it was, so that using the OS Trash costs me no safety.
14. As a user, I want Undo to work after the app restarts, so that recovery doesn't depend on keeping the app open.
15. As a user, I want Undo of only some files in a Session to work with the Trash too, so that I can bring back just the ones I need.
16. As a user, I want Redo to send the file to the Trash again, so that Redo means the same as it does for any other Operation.
17. As a user who emptied the Trash, I want the Session to stay in History with those files marked as no longer recoverable, so that the record stays true.
18. As a user who emptied the Trash, I want the rest of that Session to stay undoable, so that I only lose what I actually deleted.
19. As a user who restored a file by hand from the Trash, I want Undo to say the file is no longer in the Trash, so that it doesn't look like a failure of the app.
20. As a user, I want Undo not to overwrite a file now at the original path, so that bringing one file back never destroys another.
21. As a user whose app crashed mid-Run, I want a file whose new location wasn't recorded marked "may be in the Trash, restore it by hand", so that the app never guesses and restores the wrong file.
22. As a macOS user, I want to be asked for Full Disk Access only when an Undo needs it, if it's needed at all, so that a tidying app doesn't ask for that much up front.
23. As a macOS user who declines Full Disk Access, I want those Operations skipped with a Note to use Put Back in Finder, so that I still have a way to get them back.

### Wording

24. As a macOS or Linux user, I want the app to say "to Trash", so that it matches my OS.
25. As a Windows user, I want the app to say "to Recycle Bin", so that it matches my OS.
26. As a user, I want the app to never claim space was "freed", so that what it tells me is true.
27. As a user, I want Review and the Session screen to say where trashed files went, so that I know where to find them.

### Deleting Sessions

28. As a user, I want **Delete all Sessions** on the Settings screen, so that I can clear my History in one go.
29. As a user, I want it to ask me first, saying how many Sessions it deletes, that I can't undo them from the app afterwards and that my files won't change, so that I can't do it by accident.
30. As a user, I want it to remove only History and the Journals, so that no file of mine is touched.
31. As a user, I want files already in the Trash to stay restorable through my OS after deleting Sessions, so that I still have a way back.
32. As a user, I want it to include interrupted Sessions, so that "all" really means all.
33. As a user, I want it unavailable during a Run or Undo, so that it can't remove a Journal in use.
34. As a user with no Sessions, I want it unavailable, so that it doesn't offer to do nothing.

### Moving from the old folder

35. As a user with an old `Files Cleaner Trash` folder, I want it left as an ordinary folder that scans pick up like any other, so that its files can be cleaned through the normal flow.
36. As a user with old Sessions, I want them still listed and undoable, so that this change doesn't take away my way back.
37. As a user with a saved config from an older version, I want the app to still start and read it, so that the update doesn't break anything.

## Implementation Decisions

- **Gated rollout.** A small test of trash and restore on all three OSes comes first (ticket 01), and the developer runs it. `Files Cleaner Trash` stays the live behaviour until it passes, then all three OSes switch together. If an OS can't restore reliably, this decision comes back to the developer, as there is no folder to fall back on.
- **One new interface in `core`: the OS Trash.** It takes a path and sends it to the System Trash, returning where it landed, or says the item can't be trashed without having touched it. A second call puts an item back from a recorded location to a path, without overwriting. There's one backend per OS, beside the existing per-OS protection modules; Run, Undo and Redo reach the OS only through this interface.
- **macOS backend:** `NSFileManager trashItemAtURL:resultingItemURL:`, as ADR 0001 says, not the `trash` crate.
- **Windows backend:** `IFileOperation` with `FOFX_RECYCLEONDELETE`. It must never let a delete happen when the drive has no Recycle Bin: check first (`SHQueryRecycleBin`) and refuse anything the progress sink reports as about to be permanently deleted. How to learn the landing path, and whether moving a file back out is a clean restore, is for ticket 01 to answer.
- **Linux backend:** the FreeDesktop Trash layout (`files/` plus an `info/*.trashinfo` record), using the home Trash or the volume's `.Trash-$uid`. Check the official spec when building it; so far only third-party docs have been read.
- **Journal.** A Trash Operation's location isn't known until after it happens, so the intent is still written first, without a destination, and the location is recorded with its outcome. Redo records the new location the same way. A Move or Create Folder is journaled as today.
- **Older Journals still read.** The header's `trashDestination` becomes optional and ignored. An old Trash Operation keeps its recorded destination, and Undo moves it back from there like any other, so old Sessions stay undoable with no folder-specific code.
- **Undo of a Trash Operation.** A missing landing location gives the existing Unrecoverable state, with a Note saying it's no longer in the Trash. The existing checks stay: no overwriting, and conflicts with newer Sessions are skipped and reported.
- **Crash recovery.** A Run-phase Trash that was interrupted: if the source still exists, it didn't happen. If the source is gone and no location was recorded, it's Unrecoverable with the Note "may be in the Trash, restore it by hand". The app never searches the Trash for a match.
- **Files that can't be trashed.** The Operation is Skipped with a Note and the file stays in place; the Run continues. When the backend can tell at Plan time, the Operation is shown as blocked in the Plan and doesn't stop the rest.
- **Full Disk Access (macOS, only if ticket 01 finds it's needed).** The app explains why and opens the relevant System Settings pane at the moment Undo needs it. If access isn't granted, those Operations are Skipped with a Note to use Put Back in Finder. It's never requested at launch.
- **What goes away.** Everything for the Holding Folder: the name and the planning into it, the scanner's Protected Path check for it, the tidying of empty folders inside it, the `trashDestination` config field and the "systemTrash not supported" error, and the Settings section. A `trashDestination` key in an existing config is ignored, not an error.
- **Plan and Session wording.** Trash Operations no longer have a destination path within the Scan Root. Screens get the place name per OS ("Trash" or "Recycle Bin") in one place and don't say "freed" anywhere.
- **Delete all Sessions.** A new `core` entry point takes the existing Sessions lock (so it reports Busy during a Run or Undo) and removes every Journal. It touches only the app's own data folder, never a Scan Root, the Trash or anything named in a Journal. A Tauri command exposes it. The button lives on the Settings screen only, is unavailable with no Sessions, and confirms with the count: "Delete all 12 Sessions? You won't be able to undo them from the app. Your files won't change." There is no per-Session delete.
- **ADRs and glossary.** ADR 0001 goes from Deferred to accepted. ADR 0002 keeps "never deletes permanently", and "undoable" now holds until the user empties the Trash or deletes the Sessions. `GLOSSARY.md` drops Trash Destination and Holding Folder and adds System Trash. All are already updated; the README changes with the switch (ticket 02).

## Testing Decisions

- A good test goes through what callers use and checks what happens to files and Sessions, not how it's done internally. Per the current approach, one or two best-case tests per ticket; fuller coverage comes later.
- **Seam:** `core`'s public Run, Undo, Redo and Session entry points, with one fake: an OS Trash that moves items into a temporary folder and returns where they landed, or refuses. Tests never touch a real Trash.
- **Real backends** are checked by the developer's three-OS run (ticket 01), not by automated tests.
- **Prior art:** the Session tests in `core`'s integration tests, which already run a Plan on a temporary folder and check Undo and Redo. Those that name `Files Cleaner Trash` move to the fake at the switch.
- **Tests to add:**
  - Trashing a file, then Undo, puts it back; removing it from the fake Trash first gives Unrecoverable and leaves the rest of the Session undoable.
  - When the fake refuses a file, it stays in place and is Skipped, and the rest of the Plan runs.
  - An old-format Journal with a recorded destination still reads and undoes.
  - Delete all Sessions empties the list, and files stay where they are.

## Out of Scope

- An "Empty" action, or any automatic clean-up (by age, on quit, or on low disk space).
- A choice of Trash destination, including a user-picked folder.
- Moving the contents of old `Files Cleaner Trash` folders to the System Trash.
- Deleting a single Session (audit X6 beyond the all-at-once delete).
- Moves across disks (audit X3), and partial runs for blocked Moves (audit X2).
- Showing how much space is sitting in the Trash.

## Further Notes

- This resolves audit finding X1 (`../user-control-audit/audit.md`). ADR 0002's Consequences (no cross-disk moves, no overwriting on Undo) still stand.
- `user-control-audit` ticket 01 is marked resolved and changes "freed" to "to Files Cleaner Trash", so ticket 02 replaces that wording with the per-OS wording here.
- Research so far on the OSes comes partly from community reports: Apple's forums and a developer write-up on `~/.Trash` and Full Disk Access, and forum posts on `SHQueryRecycleBin` and network drives. Treat them as leads for ticket 01, not as settled facts.
