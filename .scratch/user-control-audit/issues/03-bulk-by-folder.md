# 03: Keep or trash every copy in a folder

**What to build:** On the Duplicates pane, "Keep every copy in…" and "Trash every copy in…", each a menu of the folders holding copies. They apply to every group the user hasn't changed by hand.

**Blocked by:** 01

**Status:** resolved

- [x] Menus list the folders holding copies
- [x] Groups changed by hand are left alone
- [x] Test: "Trash every copy in X" trashes X's copies in untouched groups only

## Comments
- 2026-11-05: Done. `setCopiesIn` and `foldersHoldingCopies` in `review-choices.js`, with `setCopy` marking a group `changedByHand`; the menus are `copiesInMenu` in `screens/review.js`. A folder means the copies directly in it, not its subfolders, matching the folder shown on each copy row. Using a menu doesn't count as changing a group by hand, so a later menu choice still applies to it. Trashing every copy of a group this way is allowed (ADR 0007), but "No copy kept" only shows on the groups listed, so past the first 50 nothing warns. Open question for the developer: warn at the top of the pane, or keep another copy when X holds the only Kept Copy.
