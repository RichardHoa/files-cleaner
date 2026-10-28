# 08: Choose the Keeper, leave copies, stop reading

**What to build:** On the merged review screen, the user decides which copy stays, can keep extra copies, and can stop reading early.
- Any copy in a Duplicate Group can be made the Keeper. The new Keeper takes the old one's place, in the same Suggested Folder or in Left where they are, with no new reading since the contents are identical. The old Keeper becomes Trash.
- Each copy other than the Keeper is **Trash** (the default) or **Leave**. A copy set to Leave stays exactly where it is and is never put into a Suggested Folder.
- The Duplicates count and "N files will move" update as copies change.
- A **Stop reading** button ends reading between files. The files read so far are grouped, every unread file goes to Left where they are, and Approve becomes possible. Stopping sends no notification and doesn't update the stored average.

See `../one-screen-review.md`: user stories 22–26, 54–55.

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] Review choices hold each copy's choice (Keeper / Trash / Leave); making a copy the Keeper moves it into the old Keeper's place and trashes the old Keeper
- [ ] The backend builds the Plan with the chosen Keeper per group; a Duplicate's Reason names the chosen Keeper
- [ ] Copies set to Leave get no Operation and aren't in any folder
- [ ] Stop reading groups the cards read so far and leaves unread files where they are
- [ ] Quiet native styling; no "AI" wording
- [ ] Test: making another copy the Keeper puts it in the old Keeper's folder and trashes the old one; a copy set to Leave is in no folder and isn't trashed

## Comments
