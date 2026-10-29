# 01: Kept Copies instead of one Keeper

**What to build:** Each copy in a Duplicate Group is Keep or Trash on its own (ADR 0007).
- Every copy has Keep / Trash; no copy is locked, and "Leave" is gone.
- The Filed Copy is the first Kept Copy in the group's order; it takes the place of the copy read among the Suggested Folders or Left where they are. Other Kept Copies are listed under Left where they are. Trashing the Filed Copy hands its place to the next Kept Copy.
- Trashing every copy is allowed; the group warns "No copy kept".
- Each copy row shows its folder (relative to the Scan Root) and modified date; its right-click menu offers Open and Show in folder.
- "freed" becomes "to Files Cleaner Trash".

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Review choices hold Keep/Trash per copy; Filed Copy derived from them
- [ ] Backend accepts a group with no Kept Copy (Reason names none)
- [ ] Copy rows show folder and date; Open in the right-click menu
- [ ] Test: keeping two copies files the first and lists the second under Left; trashing the Filed Copy hands its place to the next

## Comments
- 2026-10-29: Notes from reading the code, before any change:
  - UI: `setCopy`, `planDuplicates`, `initialDuplicates`, `placeKeepers` in `src/review-choices.js`; `duplicateCopy` / `duplicateGroup` / `duplicatesPanel` in `src/screens/review.js` (Keeper lock at the `disabled:` on the segments; "freed" in two places). The existing test in `tests/review-choices.test.js` covers Keeper/Leave and needs replacing.
  - Backend: `DuplicateChoice { keeper, trash }` in `core/src/suggested_folders.rs` needs `keeper` optional for "no copy kept"; `Reason::Duplicate { keeper }` in `core/src/planner.rs` likewise (it's written to Journals, so old Journals must still read). Tests in `core/tests/suggested_folders.rs` and `core/tests/plan.rs` name the Keeper.
  - Grouping reads every copy but the first (`without_copies` in `src-tauri/src/lib.rs`), so the Filed Copy takes the first copy's place, as `placeKeepers` does today.
  - Open: `capabilities/default.json` only grants `opener:default`, which doesn't cover opening files; either add a scoped `opener:allow-open-path` or a small Rust command calling the opener plugin.
  - Doc comments in `src/api.js` (lines ~44, ~158) mention the Keeper.
