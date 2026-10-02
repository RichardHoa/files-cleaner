# 01: Pick a folder and scan it safely

**What to build:** The user opens the app, picks a Scan Root with the native folder picker, and sees every file under it in a Scan results table (name, extension, size, modified, created, path), with a count of skipped Protected Paths. The config file is created with example Rules on first launch, can be opened from the Home screen, and an invalid config shows its error with a line number. This ticket also lays the foundations every later ticket builds on: a Rust core library with no Tauri dependency (ADR-0003), thin Tauri commands mirroring the core API, the dialog plugin, and the plain-JS UI split into screen modules.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The core library exposes `load_config` and `scan` and has integration tests against temp directories (`tempfile`, dev-dependency only)
- [ ] The scan includes every subfolder, never follows symlinks or aliases, and skips hidden files and folders
- [ ] Packages (`.app`, `.pages`, …) appear as one item with their total size and are never looked inside
- [ ] Built-in macOS Protected Paths (system folders, dev folders, app libraries, Finder metadata files, the `Files Cleaner Trash` Holding Folder) and the config's Never-Touch Paths are skipped, and the skipped count is reported
- [ ] Picking a Protected Path as the Scan Root is refused with an explanation
- [ ] The config file is created in the app data folder with the example from the spec on first launch, and is re-read on every use
- [ ] Invalid config JSON produces an error with line and column; unknown `trashDestination` or `duplicates` values and Rule targets inside Protected Paths are rejected
- [ ] The Home screen has Pick folder and Open config buttons; the Scan results screen shows the table and the skipped count
- [ ] The scaffold's greet demo is removed
