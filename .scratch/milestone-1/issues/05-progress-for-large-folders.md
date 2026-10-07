# 05: Progress for large folders

**What to build:** Scanning and running on folders with thousands of files show progress instead of looking frozen. (This ticket used to include a macOS System Trash backend; that is deferred, see ADR-0001. Trash uses the Holding Folder on every OS.)

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] Scan and Run run off the UI thread and send progress events; the Scan results and Plan/Summary screens show progress (Scan progress is done: `scan_with_progress` plus a Tauri channel, shown on Home while scanning; Run progress is still to do)
- [ ] Manual check: scanning a folder with a few thousand files keeps the window responsive
