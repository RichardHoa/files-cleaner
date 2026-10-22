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

- [ ] Tauri commands for analysis (progress and cancel), regrouping, and building the Suggested Folders Plan, using the fresh scan the step is given
- [ ] The step replaces the placeholder when the models are installed; when they're missing, ticket 02's offer stays
- [ ] Cards per Suggested Folder with name editing (an empty name is a validation error), destination ("New folder in …" / "Add to **X**" / a picked place), the four choices, per-file trash or take out, summary tooltips; "Left where they are" with no tick boxes
- [ ] Approve runs the Plan as a Session that shows in History and can be undone
- [ ] Quiet native styling as in the Plan review spec; no "AI" wording; no new tests for now

## Comments
