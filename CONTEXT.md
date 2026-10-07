# Files Cleaner

A desktop tool that tidies a messy folder (such as Downloads) by finding duplicates and applying the user's own rules, always showing a plan first and keeping every change undoable.

## Language

### Scanning

**Scan Root**:
The folder the user picks to clean; everything under it (recursively) is in scope unless protected.
_Avoid_: target dir, source folder

**Protected Path**:
A path the tool leaves alone: built-in (System Folders, Disk Roots, OS Metadata Files, app libraries, developer folders such as `.git`, `node_modules` or a `target` next to `Cargo.toml`) or listed by the user as Never-Touch. The user may still pick one, or a folder inside one, as the Scan Root after a warning; only the Protected Paths inside that Scan Root are then left alone.
_Avoid_: excluded path, ignored folder

**System Folder**:
A Protected Path that belongs to the operating system or to installed apps' settings (e.g. `/System` and `~/Library` on macOS, `C:\Windows` and `AppData` on Windows, `/usr` and `~/.config` on Linux); everything inside it is protected too. Each OS has its own list.
_Avoid_: OS folder, Library folder

**Disk Root**:
The top folder of a disk (`/`, a drive such as `C:\`, a mounted volume). Only the root itself is protected, not the folders on the disk.
_Avoid_: volume, drive

**OS Metadata File**:
A file an operating system's file manager keeps inside ordinary folders (`.DS_Store`, `Thumbs.db`, `desktop.ini`, `.directory`). Every OS's metadata files are protected on every OS, since disks move between machines.
_Avoid_: Finder metadata, junk file

**Never-Touch Path**:
A Protected Path the user added themselves.
_Avoid_: pinned folder, whitelist

**Package**:
A folder whose contents only make sense together, such as an app bundle (`.app`) or a document saved as a folder (`.pages`, `.sparsebundle`); treated as one indivisible item on every OS, wherever the folder came from.
_Avoid_: bundle

### Duplicates

**Duplicate Group**:
A set of two or more files with byte-identical contents, regardless of name or location.
_Avoid_: dupes, clones

**Keeper**:
The one file in a Duplicate Group that stays: by default the newest, judged by modified date, then created date, then shortest path.
_Avoid_: original, master copy, oldest

**Duplicate Handling**:
A setting for what happens to the non-Keeper copies: trash them, let Rules treat them like any other file, or leave them alone.
_Avoid_: dedupe mode

### Planning

**Rule**:
A user-written instruction that matches files and says what Operation to apply to them.
_Avoid_: filter, policy

**Plan**:
The full list of proposed Operations for one Scan Root, shown as a before/after tree for the user to approve, untick, or cancel.
_Avoid_: preview, proposal

**Operation**:
One atomic file-system change: Move, Rename, Trash, or Create Folder. The tool never edits file contents and never deletes permanently.
_Avoid_: action, step, task

**Trash** (Operation):
Taking a file out of the user's way without destroying it, sending it to the Trash Destination.
_Avoid_: delete, remove

**Trash Destination**:
A setting for where Trash sends files: a **Holding Folder** created inside the Scan Root, or (not yet supported) the **System Trash**, the OS's own trash can.
_Avoid_: recycle bin, quarantine

**Clash Suffix**:
The ` (1)`, ` (2)`, … added before the extension when a file's new name is already taken (`notes (1).pptx`); the same style on every OS. Names that differ only in letter case count as taken.
_Avoid_: Finder-style suffix, copy suffix

### History

**Session**:
One approved Plan being carried out, from the first Operation to the last; the unit that Undo reverses.
_Avoid_: run, job, cleanup

**Journal**:
The record of a Session's Operations, each written down before it happens, so the Session can be undone even after the app restarts.
_Avoid_: log, history file

**Undo**:
Reversing every completed Operation of a Session, in reverse order, using its Journal.
_Avoid_: rollback, revert
