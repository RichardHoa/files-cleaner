# 02: Grouping Models: offer, download, install

**What to build:** The user can get the Grouping Models onto their computer, with their consent.
- **First open:** a quiet sheet asks once whether to download the grouping models (1.8 GB), saying that nothing leaves the computer. "Not now" is remembered and never asked again at launch.
- **The download:** runs in the background while the user scans and removes duplicates.
  - It checks free space first (total plus about 10%) and refuses with a clear message if there isn't enough.
  - It resumes after an interruption and can be paused, resumed or cancelled.
  - A model counts as installed only once its SHA-256 matches.
- **Suggested Folders step without the models:** it shows the install offer, or the running download's progress, and a note that sorting by kind, name and date is coming.

Ticket 03 reads the models from the same folder, and ticket 04 uses the same "installed" check. See `../spec.md`: *Grouping Models*, user stories 1–15. ADR-0004.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The two official Qwen GGUFs are pinned: a URL at a fixed revision, a size and a SHA-256 each
- [ ] Downloads go into a models folder in app data, through a partial file resumed with HTTP Range requests; checksum verified before it counts as installed
- [ ] Status (not installed / downloading with progress / paused / installed) plus start, pause, resume, cancel and remove, as Tauri commands
- [ ] Free-space check before starting, with a clear message
- [ ] First-open sheet with **Download** / **Not now**, shown once, its answer remembered in app data
- [ ] The Suggested Folders step branches on "models installed": the offer or progress when they're missing (the fallback grouper is out of scope); the existing placeholder when they're installed, until ticket 04
- [ ] Quiet native styling and no "AI" wording; no tests for now

## Comments
