# The duplicates run on their own, from the review screen

The review screen keeps the duplicates and the Suggested Folders together, but each is now approved with its own button and run as its own Session: the Duplicates pane trashes the copies marked Trash, and the header's button runs the Suggested Folders and the Emptied Folders. One button doing both was confusing, and the copies can now be trashed while the files are still being read. This replaces the "one Session" half of ADR 0005; the grouping still reads from the first scan, before any copy is removed.

## Consequences

- History shows a Session for the duplicates and another for the folders.
- A copy trashed while the reading runs is read by its name only, if the reading hadn't reached it yet.
- Once the copies are trashed, the folder Plan is built with them again, only so the folders they leave empty are offered; their Operations aren't run twice.
