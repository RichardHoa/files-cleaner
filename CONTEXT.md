# Files Cleaner

A desktop tool that tidies a messy folder (such as Downloads) by finding duplicates and applying the user's own rules, always showing a plan first and keeping every change undoable.

## Language

### Scanning

**Scan Root**:
The folder the user picks to clean; everything under it (recursively) is in scope unless protected.
_Avoid_: target dir, source folder

**Protected Path**:
A path the tool must never read into or change: built-in (system folders, app libraries, `.git`, `node_modules`, `.DS_Store`) or listed by the user as Never-Touch.
_Avoid_: excluded path, ignored folder

**Never-Touch Path**:
A Protected Path the user added themselves.
_Avoid_: pinned folder, whitelist

**Package**:
A folder that macOS presents as a single file (an `.app`, a `.pages` document); treated as one indivisible item.
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
A setting for where Trash sends files: the **System Trash** (the OS trash can) or a **Holding Folder** created inside the Scan Root.
_Avoid_: recycle bin, quarantine

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
