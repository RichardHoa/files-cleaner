# 05: System Trash and progress for large folders

**What to build:** Users who set `trashDestination` to `systemTrash` get files sent to the real macOS Trash, still fully undoable. Scanning and running on folders with thousands of files show progress instead of looking frozen.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] The System Trash backend calls NSFileManager `trashItemAtURL:resultingItemURL:` through `objc2` / `objc2-foundation` (not the `trash` crate, per ADR-0001) and journals the resulting path
- [ ] Undo moves the file back from that path, or reports it as unrecoverable if the Trash was emptied
- [ ] Undo uses the Trash Destination recorded in the Session, not the current config
- [ ] A macOS-only round-trip test exists and is ignored by default (run with `cargo test -- --ignored`)
- [ ] Scan and Run run off the UI thread and send progress events; the Scan results and Plan/Summary screens show progress
- [ ] Manual check: scanning a folder with a few thousand files keeps the window responsive
