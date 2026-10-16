# 02: Session detail and Plan

**What to build:** The Session detail screen shows what happened at a glance: stat cards (Operations by kind, size moved to Trash, how many undone), a done/undone progress bar, and Operations grouped by kind, each row showing where the item was and where it is now with an arrow, its state in colour, and Show in Finder plus Undo or Redo. Pressing Run on a Plan lands straight on this screen with a "Done ✓" banner; the separate post-Run summary screen is removed. The Plan opens on a "By Reason" tab: one Duplicates card where each Duplicate Group shows its Keeper marked "kept ✓" with its copies below, and one card per Rule, with tick boxes per Operation, card and group, and a summary bar that follows the ticks. A "Tree" tab keeps the before/after trees, with removed items struck through in red on Before and new items in green on After, and chains of single-child folders collapsed into one row. Both tabs share one set of ticks. See `../spec.md`: user stories 31–53.

**Blocked by:** 01: Look and feel, Home tiles, History (reuses its colour tokens, icons, status pills and History navigation)

**Status:** wontfix (replaced by `plan-review` and `session-review`)

### Session detail
- [ ] The header shows the Scan Root with a folder icon and the status pill
- [ ] Stat cards: count per Operation kind, size "moved to Trash" (never "freed"), count undone
- [ ] A progress bar of done versus undone Operations; Undo all and Redo all next to the cards
- [ ] Operations grouped by kind under a heading with a count ("Trashed (12)"); each row: icon, file name, size, from → to, state in colour (done, undone, skipped), Show in Finder, Undo or Redo
- [ ] The back button returns to History (same page) when opened from there, Home otherwise
- [ ] Run goes straight to Session detail with a "Done ✓" banner; the old summary screen is removed and nothing else refers to it

### Structured Reason
- [ ] Each planned Operation carries a Reason instead of free text: Duplicate (Keeper's path relative to the Scan Root, and a Duplicate Group id) or Rule (the Rule's name); the UI writes the sentence
- [ ] Core integration tests: a duplicate's Operation names its Keeper and group; copies in the same Duplicate Group share the group id

### Plan
- [ ] A summary bar shows ticked count and size by kind, and updates on every tick
- [ ] A "By Reason" tab (the default): one Duplicates card with count and size; inside, each Duplicate Group shows its Keeper marked "kept ✓" and its copies with tick boxes; one card per Rule titled with its name
- [ ] "Tick all" on every card and every Duplicate Group behaves like ticking a folder does today
- [ ] A "Tree" tab with the before/after trees side by side: removed items struck through in red on Before, new items green on After; chains of single-child folders collapsed into one row on both sides
- [ ] Both tabs share one set of ticked Operations; switching tabs never changes what Run will do
- [ ] Cancel still returns to the Scan results; nothing changes until Run
- [ ] `node:test` tests for grouping by Reason (Keeper first, counts and sizes per card and group), collapsing folder chains, and ticking cards or groups

### Verification
- [ ] `cargo test` for the core and `npm test` pass
- [ ] Checked by running the app in light and dark appearance, including a Partly undone Session

## Comments

Closed on 2026-10-16 without being built. `.scratch/plan-review/spec.md` replaces the Plan part (cards and list instead of By Reason plus Tree tabs; the tree is dropped). `.scratch/session-review/` replaces the Session detail part (state cards, list and pane instead of stat cards and per-row buttons). Both are built.
