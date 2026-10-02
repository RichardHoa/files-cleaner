# Milestone 1: Scan, dedupe, preview, undo (macOS)

Status: ready-for-agent

## Problem Statement

My Downloads folder (and others like it) is a mess: installers I no longer need, the same PDF saved three times under different names, lecture slides scattered between subfolders. Cleaning it by hand is slow and scary: I'm never sure I'm deleting the right copy, and once a file is gone or moved I can't easily put things back. Existing tools either cost money or act without showing me what they're about to do.

## Solution

A macOS desktop app where I pick a folder, the app scans it, finds exact duplicates and applies rules I wrote myself (e.g. `.dmg` → Trash, `.pptx` → Study folder), and then shows me a before/after tree of every proposed change. I tick or untick individual changes, then run them. Nothing is ever permanently deleted: "trashed" files go either to the macOS Trash or to a Holding Folder inside the folder I picked, whichever I configured. Every change is written to a Journal before it happens, so I can undo a whole Session at any time, even after closing and reopening the app.

## User Stories

### Picking and scanning

1. As a user, I want to pick a folder with the native macOS folder picker, so that I can choose which Scan Root to clean.
2. As a user, I want the scan to include every subfolder of the Scan Root, so that duplicates and stray files hidden in nested folders are found.
3. As a user, I want to see every scanned file with its name, extension, size, modified date, created date and path, so that I know exactly what the app found.
4. As a user, I want the scan to never follow symlinks or aliases, so that it never wanders outside the folder I picked or loops forever.
5. As a user, I want hidden files and folders (dot-files) skipped, so that app and tool state inside my folders is never disturbed.
6. As a user, I want macOS Packages (`.app`, `.pages`, …) treated as one item with their total size, so that the app never reaches inside them and breaks them.
7. As a user, I want to see how many Protected Paths were skipped during the scan, so that I trust the app is respecting them.
8. As a user, I want the app to refuse to use a Protected Path as the Scan Root and tell me why, so that I can't accidentally point it at a system folder.
9. As a user, I want the scan to stay responsive on folders with thousands of files, so that the app doesn't look frozen.

### Protected Paths

10. As a user, I want system folders (`/System`, `/Library`, `/Applications`, `/usr`, `/bin`, `/sbin`, `/private`, the `/Volumes` roots, `~/Library`) never touched, so that cleaning can't damage my Mac.
11. As a developer, I want `.git`, `node_modules`, `.venv`, `__pycache__` and a `target` folder next to a `Cargo.toml` never touched, so that my projects keep working.
12. As a photographer or music listener, I want app libraries (`*.photoslibrary`, `*.musiclibrary`, `*.tvlibrary`, `iTunes/`, `Music/Music/`, `*.lrcat`, `*.lrdata`) never touched, so that my catalogues aren't corrupted.
13. As a user, I want `.DS_Store`, `Icon\r` and `.localized` files never touched, so that Finder metadata stays intact.
14. As a user, I want to list my own Never-Touch Paths in the config, so that folders I care about are left exactly as they are.
15. As a user, I want the app's own Holding Folder treated as a Protected Path, so that trashed files are never re-scanned or re-planned.

### Duplicates

16. As a user, I want files with byte-identical contents grouped into a Duplicate Group even when their names differ, so that renamed copies are caught.
17. As a user, I want duplicate detection to be fast, comparing sizes first, then a partial hash, then a full hash, so that large folders don't take forever.
18. As a user, I want empty (0-byte) files left out of duplicate detection, so that unrelated empty placeholders aren't treated as copies.
19. As a user, I want the newest copy kept as the Keeper (latest modified date, then latest created date, then shortest path, then alphabetical order), so that the result is predictable and I keep the most recently used copy.
20. As a user, I want a Duplicate Handling setting (`trash`, `followRules`, `leave`) controlling what happens to non-Keeper copies, so that the app follows my preference.
21. As a user, I want each non-Keeper copy to appear as its own item in the Plan, so that I can untick individual copies I want to keep.

### Rules and config

22. As a user, I want to write Rules in a JSON config file, so that the app organises files the way I want without needing a rules UI yet.
23. As a user, I want a Rule to match by file extension and/or a name glob, so that I can target e.g. all `.dmg` files or all `Screenshot*` files.
24. As a user, I want a Rule to trash matching files, so that junk like old installers is cleared out.
25. As a user, I want a Rule to move matching files to a folder (an absolute path, a `~/` path, or a path relative to the Scan Root), so that files end up where they belong.
26. As a user, I want a Rule to rename files while moving them, using `{name}`, `{ext}` and `{modified}` placeholders, so that I can e.g. prefix screenshots with their date.
27. As a user, I want the first matching Rule (in file order) to win, so that I can control priority just by ordering my Rules.
28. As a user, I want Rules to apply only to files and Packages, at any depth, so that whole folders aren't moved by surprise.
29. As a user, I want the config created with example Rules on first launch, so that I have a working starting point.
30. As a user, I want an "Open config" button, so that I can find and edit the file easily.
31. As a user, I want the config re-read every time I build a Plan, so that my edits take effect without restarting the app.
32. As a user, I want a clear error with a line number when my config JSON is invalid, so that I can fix it quickly instead of getting a broken Plan.
33. As a user, I want Rules that target a Protected Path rejected, so that a typo can't move files into system folders.

### Plan

34. As a user, I want to see the proposed Plan as a before/after tree, so that I understand how my folder will look afterwards.
35. As a user, I want every Operation (Move, Rename, Trash, Create Folder) shown as a separate item with a tick box, so that I can approve or untick each one.
36. As a user, I want to tick or untick a whole folder in the tree, so that I can accept or reject many items at once.
37. As a user, I want to see totals (number of Operations by type, bytes to be trashed), so that I can judge the size of the cleanup.
38. As a user, I want Create Folder Operations added automatically when a Rule's target folder doesn't exist, so that I don't have to create folders myself.
39. As a user, I want name clashes resolved with Finder-style suffixes (`notes (1).pptx`), both against existing files and between items in the same Plan, so that nothing is ever overwritten.
40. As a user, I want folders that would be emptied by the Plan offered as "Trash empty folder" items, unticked by default, so that I decide whether leftover empty folders are removed.
41. As a user, I want moves to a different disk shown as errors that can't be ticked, so that the app never silently copies and deletes.
42. As a user, I want to cancel a Plan without anything happening, so that looking at a Plan is always safe.

### Running a Session

43. As a user, I want to choose (in the config) whether Trash goes to the macOS System Trash or to a Holding Folder inside the Scan Root, so that I control where removed files end up.
44. As a user, I want the Holding Folder to be a visible `Files Cleaner Trash` folder, with one subfolder per Session that keeps each file's original relative path, so that I can find, inspect and empty it myself.
45. As a user, I want every Operation written to the Journal before it happens, so that a crash or power loss never leaves the app unaware of what it did.
46. As a user, I want each file checked right before its Operation (still exists, same size and modified date) and skipped if it changed, so that a file I edited or re-downloaded after planning isn't acted on blindly.
47. As a user, I want an "empty folder" item trashed only if the folder is truly empty at run time (or holds only `.DS_Store`), so that nothing new placed there is lost.
48. As a user, I want a summary after the run showing done, skipped and failed Operations with reasons, so that I know exactly what happened.
49. As a user, I want the app to never permanently delete anything, so that every cleanup is recoverable.

### Undo

50. As a user, I want to undo a whole Session with one click from the Summary screen, so that I can immediately reverse a cleanup I don't like.
51. As a user, I want a list of past Sessions on the Home screen, each with its status (completed, undone, partly undone) and an Undo button, so that I can reverse cleanups days later.
52. As a user, I want Undo to work after closing and reopening the app, so that reversing a cleanup doesn't depend on the app staying open.
53. As a user, I want Undo to reverse Operations in reverse order, so that moves, renames and folder creations unwind correctly.
54. As a user, I want Undo to never overwrite a file that now sits at the original location, skipping and reporting that item instead, so that Undo itself can't destroy data.
55. As a user, I want Undo to report items whose trashed copy is gone (e.g. I emptied the System Trash) as unrecoverable, without failing the rest of the Undo.
56. As a user, I want folders the Session created removed on Undo only if they're empty, so that files I added since are kept.
57. As a user, I want a Session that crashed halfway through to still be undoable, with the app checking the disk for Operations that were journaled as started but never finished, so that a crash never leaves things stuck.
58. As a user, I want Undo to use the Trash Destination recorded in the Session, so that changing the setting later doesn't break undoing older Sessions.
59. As a user, I want any past Session to be undoable, not only the latest, with each Operation checked independently, so that I can fix an older mistake safely.

## Implementation Decisions

- **Target OS:** macOS only for this milestone. OS-specific parts (built-in Protected Paths, System Trash, created-date lookup, Package detection) sit behind small modules so Windows and Linux can be added later.
- **Architecture (ADR-0003):** all logic lives in a Rust core library with no dependency on Tauri. Tauri commands are a thin layer that calls the core and serialises results. The UI is plain JS ES modules split by screen and component, with no framework and no build step.
- **Core modules** (each a deep module with a small interface):
  - **Config**: parses the config JSON into a validated Config. Errors carry line and column. Expands `~`, resolves relative `moveTo` against the Scan Root, and rejects targets that are Protected Paths.
  - **Protection**: answers "is this path protected?" using the built-in macOS list, the Never-Touch Paths and the Holding Folder name. It also detects Packages and app-library Packages.
  - **Scanner**: walks the Scan Root recursively. Does not follow symlinks, skips hidden entries and Protected Paths, and treats Packages as single items. Returns a ScanResult: a list of entries (name, extension, size, modified, created, path, isPackage) plus a count of skipped paths.
  - **Duplicates**: groups entries by size, then partial hash (start and end of the file), then full hash with BLAKE3. Excludes 0-byte files and Packages. Picks the Keeper by modified date, then created date, then shortest path, then alphabetical order.
  - **Rules**: a small hand-written glob matcher plus an extension matcher, first match wins, and rename templating (`{name}`, `{ext}`, `{modified}` as YYYY-MM-DD) with no date library.
  - **Planner**: combines Duplicates, Duplicate Handling and Rules into a Plan of Operations with stable IDs. Adds Create Folder Operations for missing targets and unticked "trash empty folder" items for folders the Plan would empty. Resolves name clashes with Finder-style suffixes. Marks cross-volume moves as non-tickable errors. Exposes a before/after tree view of the Plan.
  - **Journal**: one append-only JSON-lines file per Session in the app data folder, flushed to disk with fsync after every record. The header record holds the Session ID, Scan Root, start time and Trash Destination. Each Operation is written as `intent` before it runs, then `done`, `skipped` or `failed` with a reason, plus any data Undo needs (e.g. the path the System Trash returned). Undo records go in the same file.
  - **Executor**: runs the approved Operations in order. Before each one it writes the intent record, checks the source is unchanged (size and modified date) and the destination is free (recalculating the suffix if needed), then performs it with an atomic same-volume rename, and finally writes the outcome.
  - **Trash backends:** a Holding Folder backend (`<Scan Root>/Files Cleaner Trash/<session>/<original relative path>`) and a System Trash backend (ADR-0001) that calls NSFileManager `trashItemAtURL:resultingItemURL:` and journals the resulting path.
  - **Undo**: reads a Session's Journal and reverses completed Operations in reverse order. Never overwrites, removes created folders only if they're empty, and resolves Operations left at `intent` by checking the disk. Returns an UndoSummary (restored, skipped and unrecoverable items, each with a reason).
  - **Sessions**: lists Sessions from the Journal folder with their status (completed, crashed, undone, partly undone).
- **Core public API (the test seam):** `load_config(text)`, `scan(scan_root, config)`, `build_plan(scan_result, config)`, `run(plan, approved_operation_ids, journal_dir)`, `undo(session_id, journal_dir)`, `list_sessions(journal_dir)`. The Tauri commands mirror these one-to-one.
- **Config schema:** one JSON file in the app's data folder, created with examples on first launch:
  ```json
  {
    "trashDestination": "holdingFolder",
    "duplicates": "trash",
    "neverTouch": ["~/Downloads/Keep"],
    "rules": [
      { "name": "Disk images", "match": { "extensions": ["dmg"] }, "then": { "trash": true } },
      { "name": "Slides", "match": { "extensions": ["pptx", "key"] }, "then": { "moveTo": "~/Documents/Study" } },
      { "name": "Screenshots", "match": { "nameGlob": "Screenshot*" },
        "then": { "moveTo": "Screenshots", "rename": "{modified} {name}" } }
    ]
  }
  ```
  `trashDestination` is `holdingFolder` (the default) or `systemTrash`. `duplicates` is `trash` (the default), `followRules` or `leave`. In `match`, `extensions` and `nameGlob` must both match when both are given. `then` is either `trash: true` or `moveTo` with an optional `rename`.
- **Precedence:** if a file is a non-Keeper duplicate and Duplicate Handling is `trash`, it is trashed. If it is `leave`, the file is left out of the Plan. If it is `followRules`, or the file isn't a duplicate, the first matching Rule applies.
- **Journal safety (ADR-0002):** nothing in the codebase permanently deletes a file. A cross-volume move is refused at planning time.
- **Dependencies:** add `blake3`, `objc2` + `objc2-foundation`, `tauri-plugin-dialog`, and `tempfile` (tests only). No frontend dependencies, no glob or date crates.
- **UI screens:** Home (pick folder, Open config, Session list with Undo), Scan results (file table plus skipped count, then Build Plan), Plan (before/after tree with tick boxes per Operation and per folder, totals, Run and Cancel), Summary (done, skipped and failed with reasons, plus Undo). Long-running scans and runs report progress to the UI through events.
- **One Session at a time:** the app doesn't start a run or an Undo while another is in progress.

## Testing Decisions

- **One seam:** the core library's public API, exercised against real temporary folders. Tests check what can be seen from outside: where files end up on disk, what the Plan, Summary and UndoSummary contain, and Session status. They don't check internal functions or the exact Journal byte layout (beyond hand-writing Journals for crash scenarios).
- **Scenarios to cover:**
  - The scan respects every Protected Path category, Never-Touch Paths, hidden entries, symlinks and Packages.
  - Duplicate detection: renamed copies, same size with different contents, empty files, Keeper ordering and tie-breaks.
  - Each Duplicate Handling mode.
  - Rule matching and first-match-wins; rename placeholders; relative, `~` and absolute targets; Protected targets rejected; invalid JSON reports its line.
  - Planning: Create Folder insertion, Finder-style suffixes (against disk and within the Plan), emptied-folder items unticked by default, cross-volume moves marked as errors.
  - Running: only approved Operations run; a source changed after planning is skipped; a destination taken after planning gets a new suffix; Holding Folder layout.
  - Undo round-trip: restores the exact original tree; works with a fresh call that only gets the Journal folder (the restart case); skips instead of overwriting; reports a missing trashed copy as unrecoverable; leaves non-empty created folders; handles a crashed Session from a hand-written Journal whose disk state matches.
  - Undoing an older Session after a newer one touched the same files.
- **System Trash:** one macOS-only test, ignored by default (`cargo test -- --ignored`), because it writes to the real `~/.Trash`. It checks the round trip of trashing a file, recording its new path, and undoing.
- **Not tested automatically:** the Tauri command layer and the plain-JS screens. Check them by hand: pick a folder, see the scan table, build the Plan, untick items, run, see the Summary, quit, reopen, and undo from Home.
- **Prior art:** none yet. The repo is a fresh Tauri scaffold. These tests set the pattern: Rust integration tests in the core library using `tempfile` folders.

## Out of Scope

- Windows and Linux support, and packaging with Inno Setup.
- A UI for editing Rules or Never-Touch Paths, and a toggle for the Trash Destination (all of these are set in the config file for now).
- Editing a Plan item's destination in the tree. The user can only approve or untick.
- A Ctrl+Z keyboard shortcut for Undo.
- Cross-volume moves, cloud-drive targets, and decompressing archives.
- LLM-suggested folders, scheduled cleanups, and audio/video-aware handling.
- Matching Rules by size or age.
- Removing emptied folders except through the unticked Plan items above.
- Permanent deletion of any kind, including emptying the Holding Folder.

## Further Notes

- Domain vocabulary lives in `CONTEXT.md`. Decisions are recorded in `docs/adr/0001` (System Trash via NSFileManager), `0002` (write-ahead Journal, never delete) and `0003` (plain JS UI, Rust core).
- Implementation is split into five tickets under `issues/` (01 → 02 → 03 → 04, with 05 able to start once 02 is done).
- Commits are made by the developer, not the agent.
