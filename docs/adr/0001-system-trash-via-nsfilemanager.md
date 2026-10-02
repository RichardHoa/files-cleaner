# System Trash via NSFileManager, not the `trash` crate

When the Trash Destination is the System Trash, we call `NSFileManager trashItemAtURL:resultingItemURL:` directly (through `objc2-foundation`) instead of using the popular `trash` crate. The crate cannot restore items on macOS and does not report where a trashed file landed, yet Undo needs exactly that path, so we record it in the Journal. If the user empties the System Trash, those Operations are reported as unrecoverable on Undo.
