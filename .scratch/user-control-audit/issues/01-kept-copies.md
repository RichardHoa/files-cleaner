# 01: Kept Copies instead of one Keeper

**What to build:** Each copy in a Duplicate Group is Keep or Trash on its own (ADR 0007).
- Every copy has Keep / Trash; no copy is locked, and "Leave" is gone.
- The Filed Copy is the first Kept Copy in the group's order; it takes the place of the copy read among the Suggested Folders or Left where they are. Other Kept Copies are listed under Left where they are. Trashing the Filed Copy hands its place to the next Kept Copy.
- Trashing every copy is allowed; the group warns "No copy kept".
- Each copy row shows its folder (relative to the Scan Root) and modified date; its right-click menu offers Open and Show in folder.
- "freed" becomes "to Files Cleaner Trash".

**Blocked by:** none

**Status:** resolved

- [x] Review choices hold Keep/Trash per copy; Filed Copy derived from them
- [x] Backend accepts a group with no Kept Copy (Reason names none)
- [x] Copy rows show folder and date; Open in the right-click menu
- [x] Test: keeping two copies files the first and lists the second under Left; trashing the Filed Copy hands its place to the next

## Comments
- 2026-10-29: Notes from reading the code, before any change:
  - UI: `setCopy`, `planDuplicates`, `initialDuplicates`, `placeKeepers` in `src/review-choices.js`; `duplicateCopy` / `duplicateGroup` / `duplicatesPanel` in `src/screens/review.js` (Keeper lock at the `disabled:` on the segments; "freed" in two places). The existing test in `tests/review-choices.test.js` covers Keeper/Leave and needs replacing.
  - Backend: `DuplicateChoice { keeper, trash }` in `core/src/suggested_folders.rs` needs `keeper` optional for "no copy kept"; `Reason::Duplicate { keeper }` in `core/src/planner.rs` likewise (it's written to Journals, so old Journals must still read). Tests in `core/tests/suggested_folders.rs` and `core/tests/plan.rs` name the Keeper.
  - Grouping reads every copy but the first (`without_copies` in `src-tauri/src/lib.rs`), so the Filed Copy takes the first copy's place, as `placeKeepers` does today.
  - Open: `capabilities/default.json` only grants `opener:default`, which doesn't cover opening files; either add a scoped `opener:allow-open-path` or a small Rust command calling the opener plugin.
  - Doc comments in `src/api.js` (lines ~44, ~158) mention the Keeper.
- 2026-11-05: Trash is moving to the System Trash (`../../os-trash/spec.md`). "to Files Cleaner Trash" is right until that switch; whichever ticket lands second ends on "to Trash" / "to Recycle Bin", never "freed".
- 2026-11-05: Done. Each copy is "keep" or "trash" in `review-choices.js`; `filedCopy` derives the Filed Copy, and `placeCopies` gives it the group's place, lists other Kept Copies under Left (they can't be selected or dragged, per ADR 0007) and remembers the place while no copy is kept. `kept` replaces `keeper` on `DuplicateChoice` and `Reason::Duplicate`, optional on both. Correction to the note above: Journals don't store a Reason (a record holds `planned: SourceState`; its `reason` is a Note), so no serde alias was needed. Open is a Rust command, `open_file`, that only opens files of the latest scan, so no new capability. `Found.keeper` and `keeper_order` in `duplicates.rs` are left for ticket 02. Known and left as is: a first copy in a subfolder has no place to hand over, so once it's trashed the next Kept Copy isn't listed under Left; a sole Kept Copy trashed from its folder row or "Trash all" doesn't show "No copy kept"; putting a copy back after no copy was kept adds it at the end of its folder.
