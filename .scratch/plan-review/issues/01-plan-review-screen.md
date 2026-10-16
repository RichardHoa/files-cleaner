# 01: Plan review screen: cards by Reason, list and detail pane

**What to build:** Replace the Plan screen's Before/After trees with the layout chosen from prototype variant B. At the top, a header with the moving count, a reassurance line, and Cancel / "Move N files". Then one card per Reason (today only Duplicates) reading *what → where*, with Include all; the cards double as tabs. Then the chosen group's files, one line each, biggest first, with search, sort, a Show filter and paging, and beside them a detail pane holding only *why this file moves* and *where the original is*. The core supplies each Operation's `destination` and `keeper`, so the UI never rebuilds paths. Styling follows the spec's *Visual direction*: native and quiet, **not AI-looking**. See `../spec.md`: all user stories, Implementation Decisions, Testing Decisions.

**Blocked by:** None (can start immediately)

**Status:** resolved

### Core
- [x] `Operation` gains `destination: Vec<String>` (holding folder / This Session / relative path for Trash) and `keeper: Option<Vec<String>>` (set for duplicates); `reason` stays
- [x] `Plan.before`, `Plan.after`, `TreeNode` and its re-export are removed
- [x] `tests/plan.rs`: tree assertions replaced by `destination` / `keeper` assertions; stable-ID test still passes

### View logic (pure module, `node:test`)
- [x] Grouping by Reason (Duplicates now; a Rule group can be added later without restructuring)
- [x] Case-insensitive search over the path from the Scan Root; sort by size (default), name, folder; Show All / Ticked / Unticked; pages of 50 with "Show 50 more" / "Show all N"
- [x] The selected file falls back to the first visible file when filters hide it
- [x] `plan-selection.js`: `totals` kept; tree-based `folderState` / `toggleFolder` replaced by all / some / none and toggle over a list of Operation IDs
- [x] "N copies" from Operations sharing a `keeper`

### Screen
- [x] Header: "{N} files will move" / "Nothing will change", the reassurance line, Cancel, "Move {N} files" / disabled "Nothing ticked"; all live as ticks change
- [x] Duplicates card: ticked count and size → "Files Cleaner Trash, a folder inside {root}", one sentence, tri-state Include all, visibly the current card; no tick box nested in a `<button>`
- [x] List rows: tick box, name (+ "N copies"), folder keeping its end with the full path on hover (root-level files show "{root}/"), size; unticked rows stay, marked as left out; "No files match."
- [x] Detail pane: name, size, "An exact copy of a newer file, so this one goes to Files Cleaner Trash.", "Original: {root}/{keeper}", "Move this copy" tick box in step with the list; opens on the first file
- [x] Keyboard: ↑/↓ move the selection, Space ticks, Tab reaches every control; every tick box has an accessible name
- [x] Empty Plan message, Cancel and Run behave as today
- [x] Visual direction followed: existing tokens only, flat surfaces, hairline dividers, colour only where it means something; checked in light and dark

### Clean-up
- [x] Tree styles removed from `styles/plan.css`
- [x] `src/prototype-plan.html`, `src/prototype-plan.js` and the `prototype:plan` script in `package.json` deleted
- [x] `CONTEXT.md` **Plan** entry updated: no longer "a before/after tree"

### Verification
- [x] `cargo test` for the core and `npm test` pass
- [x] Checked by running the app on a small and a 100+ duplicate folder, in light and dark appearance
- [x] Not committed: left for the developer to review and commit
