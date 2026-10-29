# One Plan and one Session for duplicates and Suggested Folders, grouping before anything is removed

The duplicate step and the Suggested Folders step are reviewed on one screen, approved as one Plan and carried out as one Session. The Scan results screen and the rescan between the steps are gone. Grouping now reads from the first scan, before any duplicate is removed: it reads only the files that would remain (non-duplicates and Keepers, not copies marked for Trash, not files matched by a Rule), and a copy the user decides to leave stays where it is, outside any Suggested Folder. We chose this so the user does everything on one screen and can work through the duplicates while the slow reading runs, instead of waiting through two Plans, two Sessions and a rescan.

## Considered Options

- **Two steps, two Sessions, with a rescan between them (ticket `pipeline/02`):** rejected. It kept the duplicates undoable on their own, but it meant an extra screen and a wait in the middle, and users found the flow confusing.

## Consequences

- Replaces `pipeline/02`'s rule that grouping never uses the scan from before the duplicate Session.
- Undoing only the duplicates, or only the folder moves, now goes through per-Operation Undo; History shows one Session.
- Changing the Keeper after reading needs no new reading, as the copies are byte-identical: the new Keeper takes the old one's place.
- The single Keeper and the "leave" choice for copies are replaced by Kept Copies and the Filed Copy: see ADR 0007.
