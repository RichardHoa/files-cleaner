# Write-ahead Journal per Session; the tool never deletes permanently

Every Operation is appended to the Session's Journal (one JSON-lines file per Session, fsynced) as an intent *before* it touches the disk, and marked done, skipped or failed afterwards. Trash always moves files somewhere recoverable (the System Trash, ADR-0001); nothing calls a permanent delete. This is the app's core safety guarantee: any Session can be undone after a crash or restart, until the user empties those files from the System Trash or deletes the Sessions, and future features must not add "just delete it" shortcuts. A Trash's location is only known once it has happened, so it's recorded with the outcome, after the intent. SQLite was considered and deferred until the session-timeline feature needs querying.

## Consequences

- Cross-volume moves (which are copy + delete underneath) are refused in Milestone 1 rather than silently deleting the source.
- Undo never overwrites: conflicting Operations are skipped and reported.
