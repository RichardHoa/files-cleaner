# Any number of Kept Copies, only the Filed Copy is sorted

A Duplicate Group no longer has one Keeper: each copy is kept or trashed on its own, the Default Copy Rule (a setting) picks which starts out kept, and trashing every copy is allowed with a warning, as Undo can bring it back. Only one copy of a group is read, so only one Kept Copy, the Filed Copy, goes into the Suggested Folders; the others stay where they are, listed with the files left where they are. We chose this because a second copy is usually kept for where it is (a backup, another project), and filing both into one folder would only make `report.pdf` and `report (1).pdf` side by side, a duplicate again. This replaces ADR 0005's Keeper and its "Leave" choice for copies.

## Considered Options

- **Every Kept Copy follows the copy read into its folder:** rejected, as it recreates the duplicate inside the new folder.
- **At least one copy must be kept:** rejected as a hard rule; it only warns.
