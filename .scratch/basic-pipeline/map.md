# Basic pipeline: scan, remove duplicates, Suggested Folders

The basic goal is that a user can do this, end to end, on macOS, Windows and Linux:

1. **Scan report.** Pick a Scan Root and see what's in it. *Built.*
2. **Duplicates.** Approve a Plan that trashes non-Keeper copies; its own Session, undoable on its own. Skippable.
3. **Suggested Folders.** After an automatic rescan, the tool proposes Suggested Folders for the loose items in the Scan Root. For each one the user creates it (in the Scan Root by default, or somewhere they pick), trashes all its files, or skips it; within it they can trash single files or take them out. Its own Session.

## Notes

- Terms are in `CONTEXT.md` (Suggested Folder, Reason, Session, …).
- Each step is a separate Plan and Session, so grouping never sees duplicate copies, and each step can be undone without the other.

## Decisions so far

- **Separate steps** (2026-10-20): duplicates, then rescan, then Suggested Folders; each its own Plan and Session.
- **Rules frozen** (2026-10-20): code and tests stay, unused; the Duplicate Handling setting goes. → `pipeline/issues/01`
- **Suggested Folder choices** (2026-10-20): create here / create elsewhere / trash all / skip; per file: trash or take out. An empty name is a validation error, never a delete.
- **Loose items only** (2026-10-20): only items directly in the Scan Root are grouped; existing subfolders can be destinations.
- **Every OS** (2026-10-20): new code is designed and tested for macOS, Windows and Linux; the port of the existing core is done early. → `milestone-1/issues/06`
- **Cross-platform grouping tools** (2026-10-20): the prototype uses Rust with a local ONNX embedding model and keyword names, no Apple frameworks, compared with a no-ML baseline. → `content-grouping/issues/01`
- **Same disk only** (2026-10-20): moves to another disk stay blocked; copying across disks comes with cloud drives. → `milestone-1/issues/04`
- **No-extension files stay out** (2026-10-20).
- **Content grouping ships** (2026-10-22): the prototype's LLM-card grouper beat the baseline. llama.cpp runs both Grouping Models (official Qwen GGUFs, downloaded on first use with consent; Metal on Mac, Vulkan on Windows and Linux, CPU fallback); C/C++ build dependencies are allowed; the engine is its own crate. Without the models, Suggested Folders fall back to the baseline (later). → `content-grouping/spec.md`, ADR-0004
- **Settings screen** (2026-10-22): the config gets an in-app Settings screen (Trash Destination, Never-Touch Paths, grouping thresholds, Grouping Models). → `content-grouping/issues/05`

## Roadmap

1. **Foundations** (in parallel)
   - `milestone-1/issues/06`: port the core to Windows and Linux
   - `content-grouping/issues/01`: grouping prototype → developer's verdict → spec (and ADR if ML wins)
2. **Separate steps**
   - `pipeline/issues/01`: the duplicate step, duplicates only
   - `pipeline/issues/02`: step flow with rescan and a Suggested Folders placeholder
3. **Safety before big moves**
   - `milestone-1/issues/04`: emptied folders, blocked moves, undoing an older Session after a newer one touched the same files
   - `milestone-1/issues/05`: Run progress
4. **Suggested Folders in the app** (`content-grouping/spec.md`, issues 02–05): grouping engine in the core with progress and caching, Suggested Folder as a Reason in the Plan, the choices and per-file actions, existing subfolders as destinations

**Basic goal reached.**

## Fog (after the basic goal)

- **Cloud drives.** Let a cloud drive's folder be a destination for "create elsewhere"; the drive's own app syncs it. No cloud APIs or logins. On macOS these live under `~/Library/CloudStorage`, which is a System Folder today, so they need an exception. Google Drive on Windows is its own drive (`G:`), so it needs copying across disks.
- **Rules.** Revive only if a real need appears, e.g. as fixed overrides of Suggested Folders.
- **OCR and LLM naming** for Suggested Folders, if the prototype says they'd help.
- Other cleanup needs users raise: record them here before building anything.
