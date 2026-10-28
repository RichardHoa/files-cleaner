# 05: Settings screen and grouping tuning

**What to build:** The user can change how the app behaves without editing JSON.
- A Settings screen, reached from Home, has:
  - the Trash Destination;
  - Never-Touch Paths, with Add (folder picker) and Remove;
  - the two grouping thresholds, each a slider plus an exact number kept in step, with a one-line explanation and "Reset to default";
  - the smallest folder size (2–10);
  - "Read every file again": forgets what the Grouping Models read before, so the next grouping reads every file afresh (it takes as long as the first time);
  - the Grouping Models: status, Download / Pause / Cancel / Remove, and for each of the two models its name, its size on disk and the folder it is in, with "Show in Finder" (Explorer / file manager on other OSes), plus the total size. Remove deletes both, after which Download is offered again.

  "Open config file" stays as a small link. Changes save as they're made.
- The Grouping disclosure (both sliders, Reset, "Read every file again") leaves the Suggested Folders screen, as does the "Read every file again in the next step" note on the duplicate screen. There is no slider on that screen: changed settings apply the next time files are grouped.
- The grouping thresholds move from the screen's browser storage into the config file.

See `../spec.md`: *Core: config*, *UI* (Settings and the slider), user stories 45–46 and 49–64.

**Blocked by:** 02, 04

**Status:** ready-for-agent

- [ ] Config gains an optional `grouping` section (`folderThreshold`, `existingFolderThreshold`, `smallestFolder`) whose out-of-range values fail with the line-and-column config error; the engine uses these values instead of the defaults in code
- [ ] Config saving: atomic, and keeps every key the UI doesn't edit, including the frozen Rules
- [ ] Settings screen with the sections above; an invalid value shows an inline error and isn't saved; the System Trash is shown as not yet supported
- [ ] Grouping Models section shows each model's name, size on disk and folder with Show in Finder, and the total; Remove deletes both
- [ ] The Grouping disclosure and the "Read every file again in the next step" note are removed from the review screens; the thresholds are read from the config, not browser storage
- [ ] Quiet native styling; no "AI" wording
- [ ] Test: saving a changed `folderThreshold` and loading the config again gives the same value

## Comments

2026-10-28 (grilling): the live "Looser ↔ Stricter" slider on the Suggested Folders step is dropped. The grouping settings and "Read every file again" live only in Settings. Settings also shows where each Grouping Model is, its size on disk, and Remove.
