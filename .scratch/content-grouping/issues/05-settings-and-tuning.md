# 05: Settings screen and grouping tuning

**What to build:** The user can change how the app behaves without editing JSON.
- A Settings screen, reached from Home, has:
  - the Trash Destination;
  - Never-Touch Paths, with Add (folder picker) and Remove;
  - the two grouping thresholds, each a slider plus an exact number kept in step, with a one-line explanation and "Reset to default";
  - the smallest folder size (2–10);
  - the Grouping Models: status, size, Download / Pause / Cancel / Remove.

  "Open config file" stays as a small link. Changes save as they're made.
- The Suggested Folders step gets a "Looser ↔ Stricter" slider that regroups instantly and saves the new threshold.

See `../spec.md`: *Core: config*, *UI* (Settings and the slider), user stories 45–46 and 49–64.

**Blocked by:** 02, 04

**Status:** ready-for-agent

- [ ] Config gains an optional `grouping` section (`folderThreshold`, `existingFolderThreshold`, `smallestFolder`) whose out-of-range values fail with the line-and-column config error; the engine uses these values instead of the defaults in code
- [ ] Config saving: atomic, and keeps every key the UI doesn't edit, including the frozen Rules
- [ ] Settings screen with the sections above; an invalid value shows an inline error and isn't saved; the System Trash is shown as not yet supported
- [ ] Live slider on the Suggested Folders step: regroups with no model run, keeps per-folder edits for groups that still exist, saves `folderThreshold`, and maps the same way as in Settings
- [ ] Quiet native styling; no "AI" wording
- [ ] Test: saving a changed `folderThreshold` and loading the config again gives the same value

## Comments
