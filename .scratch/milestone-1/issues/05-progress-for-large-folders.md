# 05: Progress for large folders

**What to build:** Scanning and running on folders with thousands of files show progress instead of looking frozen. (This ticket used to include a macOS System Trash backend; that is deferred, see ADR-0001. Trash uses the Holding Folder on every OS.)

**Blocked by:** 02

**Status:** resolved

- [x] Scan and Run run off the UI thread and send progress events; the Scan results and Plan/Summary screens show progress (Scan progress is done: `scan_with_progress` plus a Tauri channel, shown on Home while scanning; Run progress: `run_with_progress` plus a channel on `run_plan`, shown on the Plan screen while it runs)
- [x] Manual check: scanning a folder with a few thousand files keeps the window responsive

## Comments

**2026-10-20, implemented.** `run_with_progress` reports `{ finished, total, current }` before the first Operation, at most every 100 ms, and once at the end; `run` is it with no callback. The Plan screen's status line shows "Moving 157 of 824 items…" and the item it's on. Checked in the real app on Linux (Xvfb): a 4,882-item, 3.2 GB scan, then an 824-Operation Run, with the status line repainting during the Run. Still to check by hand on a Mac and a Windows PC.
