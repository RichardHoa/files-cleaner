# Trash always goes to the System Trash; NSFileManager on macOS, not the `trash` crate

Status: Accepted (2026-11-05), replacing the earlier deferral. The Holding Folder stays live until a test of trash and restore passes on macOS, Windows and Linux (issue #5); then all three switch together and the Holding Folder is removed. If an OS can't restore reliably, this decision is reopened.

Trash has one behaviour, with no setting: files go to the OS's own trash, so the user gets their space back by emptying it, as they do for everything else. The Holding Folder kept files inside the folder being cleaned and nothing ever emptied it, so the app could never finish its job. Each OS has a backend that reports where each trashed item lands, and that location is recorded in the Journal for Undo. If the user empties the System Trash, those Operations are reported as unrecoverable on Undo.

On macOS we call `NSFileManager trashItemAtURL:resultingItemURL:` directly (through `objc2-foundation`) instead of using the popular `trash` crate. The crate cannot restore items on macOS and does not report where a trashed file landed, yet Undo needs exactly that path.

## Considered Options

- **A setting to choose the System Trash or the Holding Folder:** rejected, as it's two behaviours to build and explain, with no real need for the second.
- **Keep the Holding Folder and add a confirmed "Empty" action:** rejected, as it would break ADR 0002.
- **Keep the Holding Folder and add "Move to System Trash":** rejected, as it makes getting space back a two-step job.

## Consequences

- A file that can't go to the System Trash (such as on a network drive) stays where it is and is reported; a trash never becomes a permanent delete, and there is no fallback folder.
- Old Holding Folders are left as ordinary folders, which scans pick up like any other. Old Sessions stay undoable from the paths in their Journals.
