# Write-ahead Journal per Session; the tool never deletes permanently

Every Operation is appended to the Session's Journal (one JSON-lines file per Session, fsynced) as an intent *before* it touches the disk, and marked done, skipped or failed afterwards. Trash always moves files somewhere recoverable (System Trash or Holding Folder); nothing calls a permanent delete. This is the app's core safety guarantee: any Session can be undone after a crash or restart, and future features must not add "just delete it" shortcuts. SQLite was considered and deferred until the session-timeline feature needs querying.

## Consequences

- Cross-volume moves (which are copy + delete underneath) are refused in Milestone 1 rather than silently deleting the source.
- Undo never overwrites: conflicting Operations are skipped and reported.
