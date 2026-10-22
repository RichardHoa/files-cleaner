# 04: Suggested Folders step in the app

**What to build:** With the Grouping Models installed, the Suggested Folders step works end to end.
- After the rescan, the step reads the loose files with progress ("Reading 12 of 85 files…" and the current file). Cancel goes back with nothing changed.
- It then shows the Plan, in the Plan review layout, with one card per Suggested Folder. Each card has:
  - the editable name;
  - "New folder in {Scan Root}" or "Add to **X**";
  - the choice: create here / create elsewhere… / trash all / skip;
  - its files, each with trash or take out and the summary as a hover tooltip.
- "Left where they are" is listed last.
- Approving runs the Plan as its own Session, undoable from History.

See `../spec.md`: *UI* (Suggested Folders step), user stories 16–48.

**Blocked by:** 02, 03

**Status:** ready-for-agent

- [x] Tauri commands for analysis (progress and cancel), regrouping, and building the Suggested Folders Plan, using the fresh scan the step is given
- [x] The step replaces the placeholder when the models are installed; when they're missing, ticket 02's offer stays
- [x] Cards per Suggested Folder with name editing (an empty name is a validation error), destination ("New folder in …" / "Add to **X**" / a picked place), the four choices, per-file trash or take out, summary tooltips; "Left where they are" with no tick boxes
- [x] Approve runs the Plan as a Session that shows in History and can be undone
- [x] Quiet native styling as in the Plan review spec; no "AI" wording; no new tests for now

## Comments

Implemented on macOS (2026-10-22). `cargo check` has no warnings, `cargo test --workspace` passes and `npm test` passes (71). The UI hasn't been run yet; that's for the developer.
- Commands: `analyse_loose_files` (progress channel), `cancel_analysis`, `regroup` (tied to the scan it came from; no caller until ticket 05's slider) and `build_suggested_folders_plan`. Approving reuses `run_plan`, so the Plan runs as its own Session.
- Settings come from the defaults in code until ticket 05 adds the config `grouping` section.
- A matched group offers "Add to X" plus "New folder here" (story 32), as well as create elsewhere…, trash all and skip.
- If any Operation can't happen (such as a move to another disk under "create elsewhere"), Approve refuses the whole Plan and names the folder. It doesn't run the rest, so the user picks another place or skips.
- "N files will move" counts trashed files too, the same as the duplicate step.
- Cancel waits for the analysis to stop between files, then goes Home.
- The progress total also counts the files sampled from subfolders to describe them (as in `analyse`), so it's a little higher than the number of loose files.

