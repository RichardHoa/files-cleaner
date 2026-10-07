# System Trash via NSFileManager, not the `trash` crate

Status: Deferred. The System Trash was postponed when the app went cross-platform (macOS, Windows, Linux); for now Trash always uses the Holding Folder, on every OS. Reviving it needs a backend per OS that reports where each trashed item lands (or an equivalent receipt Undo can use to restore it), recorded in the Journal. The reasoning below still holds for the macOS backend.

When the Trash Destination is the System Trash, we call `NSFileManager trashItemAtURL:resultingItemURL:` directly (through `objc2-foundation`) instead of using the popular `trash` crate. The crate cannot restore items on macOS and does not report where a trashed file landed, yet Undo needs exactly that path, so we record it in the Journal. If the user empties the System Trash, those Operations are reported as unrecoverable on Undo.
