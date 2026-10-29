# 08: Choose the Keeper, leave copies, stop reading

**What to build:** On the merged review screen, the user decides which copy stays, can keep extra copies, and can stop reading early.
- Any copy in a Duplicate Group can be made the Keeper. The new Keeper takes the old one's place, in the same Suggested Folder or in Left where they are, with no new reading since the contents are identical. The old Keeper becomes Trash.
- Each copy other than the Keeper is **Trash** (the default) or **Leave**. A copy set to Leave stays exactly where it is and is never put into a Suggested Folder.
- The Duplicates count and "N files will move" update as copies change.
- A **Stop reading** button ends reading between files. The files read so far are grouped, every unread file goes to Left where they are, and Approve becomes possible. Stopping sends no notification and doesn't update the stored average.

See `../one-screen-review.md`: user stories 22–26, 54–55.

**Blocked by:** 07

**Status:** resolved

- [x] Review choices hold each copy's choice (Keeper / Trash / Leave); making a copy the Keeper moves it into the old Keeper's place and trashes the old Keeper
- [x] The backend builds the Plan with the chosen Keeper per group; a Duplicate's Reason names the chosen Keeper
- [x] Copies set to Leave get no Operation and aren't in any folder
- [x] Stop reading groups the cards read so far and leaves unread files where they are
- [x] Quiet native styling; no "AI" wording
- [x] Test: making another copy the Keeper puts it in the old Keeper's folder and trashes the old one; a copy set to Leave is in no folder and isn't trashed

## Comments

- 2026-10-29: Done. `setCopy` in `review-choices.js` sets a copy to Keeper, Trash or Leave; a new Keeper takes the old one's place among the Suggested Folders or the files left, and the old Keeper is trashed. A Keeper picked while files are still read is put in place of the copy read (`placeKeepers`), and shows that copy's summary. `build_review_plan` already took the chosen Keeper, so the Duplicate Reasons name it with no backend change. `stop_reading` sets a stop flag that `grouping::analyse` checks between files: the cards read so far are grouped, unread loose files go to Left, and only stored answers place files left out. The button hides once the placing pass starts; a stopped run saves no reading pace and sends no notification.
