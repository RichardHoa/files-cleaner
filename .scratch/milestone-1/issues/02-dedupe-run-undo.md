# 02: Trash duplicates, then undo the Session

**What to build:** The first full tracer bullet. From the Scan results, the user builds a Plan that proposes trashing every non-Keeper copy in each Duplicate Group, sees it as a before/after tree with tick boxes, runs it, and gets a Summary. Trashed files go to the Holding Folder. Every Operation is written to the Journal before it happens, and the user can undo the whole Session from the Summary screen or, after restarting the app, from the Session list on Home.

**Blocked by:** 01

**Status:** resolved

- [x] Duplicate Groups are found by size, then partial hash, then full BLAKE3 hash; 0-byte files and Packages are excluded; renamed copies are caught
- [x] The Keeper is the newest copy: latest modified date, then latest created date, then shortest path, then alphabetical order
- [x] `build_plan` returns a Plan with stable Operation IDs and a before/after tree; the Plan screen shows tick boxes per Operation and per folder, totals (counts by type and bytes to trash), and Run and Cancel buttons; Cancel changes nothing
- [x] `run` performs only the approved Operations, moving trashed files to `<Scan Root>/Files Cleaner Trash/<session>/<original relative path>`
- [x] Each Operation is recorded as an intent (flushed to disk) before it happens and as done, skipped or failed after it; the Session header records the Scan Root and the Trash Destination (ADR-0002)
- [x] A source that changed (missing, or a different size or modified date) between planning and running is skipped with a reason
- [x] Only one Session runs at a time
- [x] The Summary screen shows done, skipped and failed Operations with reasons, plus an Undo button
- [x] `undo` reverses completed Operations in reverse order from the Journal alone (a test proves it works from a fresh call that only gets the Journal folder), never overwrites, and reports items whose trashed copy is gone as unrecoverable
- [x] `list_sessions` and the Home Session list show each Session's status (completed, undone, partly undone) with an Undo button
- [x] Round-trip test: scan → plan → run → undo restores the original folder exactly

## Comments

Resolved after checking the code on 2026-10-16. Core: `duplicates.rs`, `planner.rs`, `executor.rs`, `journal.rs`, `sessions.rs`, with tests in `tests/plan.rs` and `tests/session.rs`. The before/after tree on the Plan screen was later replaced by `plan-review`. The Summary screen was later replaced by `session-review`: the Session screen now shows the run's result and offers Put back.
