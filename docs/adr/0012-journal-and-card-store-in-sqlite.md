---
status: accepted
---

# The Journal and the card store live in SQLite; small settings files stay JSON

The Journal records every Operation before it touches the disk, so any Session can be undone after a crash. It was one JSON-lines file per Session, and the card store was one JSON file per card, and we were writing and testing our own atomicity, durability and crash recovery for both (fsync calls, truncating half-written lines, ignoring empty files). Both now live in SQLite, which gives us all of that and can be inspected with any SQLite tool.

- **Journal:** `journal.sqlite` in the Sessions folder, with a `sessions` table for the headers and an append-only `records` table holding each record as JSON. It runs in WAL mode with `synchronous=FULL`, so each record is committed before the Operation it describes.
- **Card store:** `cards.sqlite`, with typed columns and embeddings stored as little-endian f32 blobs.
- **Settings** (the config, the Recent Scan Roots, onboarding, the reading pace and a custom model's source) stay small JSON files, replaced atomically: written to a temporary file that is fsynced, renamed over the old one, then the folder is fsynced.

## Considered Options

- **Keep JSON-lines and per-card JSON files, and only add the missing fsyncs:** rejected because it keeps us maintaining our own ACID.
- **Move the settings files into SQLite too:** rejected because there's nothing to gain for a few small files that aren't crucial.

## Consequences

- `core` is no longer pure Rust: `rusqlite` with `bundled` compiles SQLite's C into both `core` and `grouping`.
- Journals written before this change are imported the first time the database opens. Each file is deleted only after its Session is committed, and a file that can't be read stays where it is. Old card files are deleted rather than imported, since they are only a cache, so files are read again once.
- Schema changes go through `PRAGMA user_version` migrations in each database's `migrate`.
- The lock that lets one Session run at a time stays a lock file, since a Session spans many commits.
