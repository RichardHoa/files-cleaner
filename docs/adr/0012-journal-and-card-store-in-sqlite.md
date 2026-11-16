# The Journal and the card store live in SQLite; small settings files stay JSON

This partly reverses ADR 0002 and ADR 0004. The Journal moves from one JSON-lines file per Session to one SQLite database (`journal.sqlite` in the Sessions folder). It keeps a `sessions` table for the headers and an append-only `records` table holding each record as JSON. It runs in WAL mode with `synchronous=FULL`, so each record is committed to disk before the Operation it describes. The write-ahead rule and "never delete permanently" from ADR 0002 stand unchanged. Only the file format changes. The card store moves from one JSON file per card and per placing answer to `cards.sqlite`, with typed columns and embeddings stored as little-endian f32 blobs. SQLite gives us atomicity, durability and crash recovery that we were otherwise writing and testing by hand: the fsync calls, truncating half-written lines, and ignoring empty files. The database can be inspected with any SQLite tool.

The config, the Recent Scan Roots, onboarding, the reading pace and a custom model's source stay small JSON files. They aren't crucial, and they are replaced atomically: written to a temporary file that is fsynced, renamed over the old file, and then the folder is fsynced.

## Considered Options

- **Keep JSON-lines and per-card JSON files, and only add the missing fsyncs:** rejected because it keeps us maintaining our own ACID.
- **Move the settings files into SQLite too:** rejected because there's nothing to gain for a few small files.

## Consequences

- `core` is no longer pure Rust. `rusqlite` with `bundled` compiles SQLite's C into both `core` and `grouping`, which goes against ADR 0004's "core stays pure Rust".
- Journals written before this change are imported into the database the first time it opens. Each file is deleted only after its Session is committed, and a file that can't be read stays where it is. The old card files are deleted rather than imported, since they are only a cache, so files are read again once.
- Schema changes go through `PRAGMA user_version` migrations in each database's `migrate`.
- The lock that lets one Session run at a time stays a lock file, since a Session spans many commits.
