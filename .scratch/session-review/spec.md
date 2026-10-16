# Session review: what happened, where it is now, and putting things back

Status: ready-for-agent

## Problem Statement

When I open a past Session I can't quickly tell what happened. The page starts with "Session 16 Oct 2026, 14:02", the Scan Root's full path, a status pill and "112 Operations done · 12 undone". It never says in plain words what moved, where it went, or what is already back.

Every file gets two or three lines: its full path from the Scan Root, then "In Files Cleaner Trash/<Session ID>/…" repeating the same path, then sometimes a grey note. The file's own name is buried in the middle. Each line also has a "Trash" kind pill (the same on every line today) and a coloured state pill, so every line is coloured and nothing stands out. Each line has its own **Show in Finder** and **Undo**/**Redo** buttons, so a Session of 112 files has more than 200 buttons, on top of **Undo all** and **Redo all** at the top. I can't ask "which files are back?" or "what went wrong?": every state is mixed together in run order, with no search, sort or paging.

After I press Move on the Plan, I land on a separate Summary screen. It repeats most of the Session page (counts, Undo, a list of what was done) and has an **Open Session** button to get to the real page. It is one more screen with one more Undo button.

The words are the app's internal ones: "Operations", "Trashed", "Redo", "Stopped part-way". The Plan screen already says "Move 112 files".

## Solution

The Session screen is rebuilt in the same pattern as the Plan screen, around three questions about the past: *what happened*, *where is it now*, and *what about this file*.

1. **Header:** a back button (to History or Home, as today), then a heading that says what happened and keeps up as files are put back ("112 files moved to Files Cleaner Trash", "12 of 112 files put back", "All 112 files are back", "Stopped after 40 of 112 files"). Under it, the date and the Scan Root's name, then one reassuring line. One primary button: **Put all back** while anything is still moved.
2. **One card per state group, used as tabs:** **Moved** (with its size and "→ Files Cleaner Trash, a folder inside Downloads"), **Put back** ("→ where they were"), and **Needs attention** (skipped, failed, unrecoverable, or stopped part-way, with one sentence saying what that means). Groups with nothing in them aren't shown. Clicking a card lists its files below. Because the card says the state, the lines don't need state pills.
3. **List and detail pane, side by side:** on the left, the chosen group's files, one line each (tick box, name, the folder it came from, size), biggest first, with search, sort and paging as on the Plan screen. Below the list, one action for the ticked files: **Put back N files** on the Moved tab, **Move N files again** on the Put back tab. On the right, the selected file's name and size, one sentence on where it is now (or, under Needs attention, why it didn't happen), and two buttons: **Show in Finder** and a single-file **Put back** / **Move again**. Right-clicking a line offers Show in Finder, as on the Plan screen.

The Summary screen is removed. After Move, the app goes straight to the new Session screen, with the result as a short status line ("112 files moved · 0 skipped").

The field that core calls `reason` on a Session's Operations and on Run/Undo/Redo outcomes is renamed to `note`. It means "why this didn't happen", not the glossary's **Reason** ("why the Plan proposed this Operation"), and sharing a name with it causes confusion. The Journal's on-disk format does not change.

## User Stories

### Seeing the whole Session at once

1. As a user, I want the heading to say what the Session did ("112 files moved to Files Cleaner Trash"), so that I understand it without decoding counts.
2. As a user, I want the heading to change after I put files back ("12 of 112 files put back", "All 112 files are back"), so that it always describes where things are now.
3. As a user, I want a Session that stopped part-way to say so in the heading ("Stopped after 40 of 112 files"), so that I know it didn't finish.
4. As a user, I want the date and the Scan Root's name under the heading, with the full path on hover, so that I know which Session and folder this is.
5. As a user, I want one plain line saying nothing was deleted and that I can put back any file or all of them, so that I'm not afraid to look around.
6. As a user, I want one "Put all back" button, so that I can reverse the whole Session in one click without hunting for it.
7. As a user, I want "Put all back" to be hidden or disabled when nothing is left to put back, so that I can't press a button that does nothing.
8. As a user, I want the back button to take me to the History page I came from, or to Home, as it does today, so that I don't lose my place.

### State groups

9. As a user, I want a card for the files that are still moved, showing how many, their total size, and that they are in Files Cleaner Trash, a folder inside my Scan Root, so that I know where they are.
10. As a user, I want a card for the files I've put back, saying they're back where they were, so that I can see what I've already reversed.
11. As a user, I want a "Needs attention" card for files that were skipped, failed, couldn't be recovered, or were cut off by a stop part-way, so that problems are in one place instead of scattered through the list.
12. As a user, I want a card to appear only when it has files, so that I'm not shown empty groups.
13. As a user, I want clicking a card to list its files, with the current card visibly marked like a selected sidebar item, so that I know which list I'm looking at.
14. As a user, I want the first card with files to be chosen when the page opens, so that the list is useful straight away.
15. As a user, I want the cards' counts and sizes to update after I put back or move files again, so that they stay true.

### The list

16. As a user, I want one line per file with a tick box, its name, the folder it came from, and its size, so that a long Session stays easy to scan.
17. As a user, I want no pills or buttons on each line, so that the list is quiet and the file names stand out.
18. As a user, I want the list sorted biggest first by default, with name and folder as other sorts, so that I can find a file the way I think of it.
19. As a user, I want to search by file name or folder, so that I can check a specific file.
20. As a user, I want a long folder to keep its end (the nearest folder) when it doesn't fit, with the full path on hover, so that I can tell folders apart.
21. As a user, I want 50 files at a time with "Show 50 more" and "Show all N", so that a Session of thousands of files stays fast.
22. As a user, I want "No files match." when a search finds nothing, so that an empty list doesn't look broken.
23. As a user, I want a Select all tick box for the listed group, so that I can act on a whole group or most of it quickly.
24. As a user, I want "Put back N files" for the ticked files on the Moved tab, so that I can reverse just some of the Session.
25. As a user, I want "Move N files again" for the ticked files on the Put back tab, so that I can change my mind about files I put back.
26. As a user, I want the action button disabled and labelled "Nothing ticked" when nothing is ticked, so that I can't start an empty Undo or Redo.
27. As a user, I want no tick boxes or bulk action on the Needs attention tab, so that I'm not offered actions that can't happen there.
28. As a user, I want ticks cleared when I switch cards or after an action, so that I never act on files I can't see.
29. As a user, I want to move through the list with the arrow keys and tick with Space, so that I can review a big Session from the keyboard.

### This file

30. As a user, I want clicking a file to show it in the side pane, with the pane opening on the first file rather than empty, so that I can check one file without the list moving.
31. As a user, I want the pane to say where a moved file is now ("In Files Cleaner Trash, inside Downloads."), so that I can find it.
32. As a user, I want the pane to say that a file that was put back is back in its folder, so that I know it's safe.
33. As a user, I want the pane to tell me, for a file under Needs attention, what happened and why ("Skipped: a file with this name is already in Zoom/"), so that I can fix it.
34. As a user, I want "Show in Finder" in the pane to reveal the file where it is now (in the holding folder while moved, back in its folder once put back), so that I can look at it.
35. As a user, I want a single-file "Put back" or "Move again" button in the pane, so that I can act on the file I'm reading about.
36. As a user, I want right-clicking a line to offer "Show in Finder", as on the Plan screen, so that both screens behave the same.

### Results of an action

37. As a user, I want a short status line after Put back or Move again ("12 files put back · 1 skipped"), so that I know it worked.
38. As a user, I want files that Undo or Redo skipped to show up under Needs attention with why, so that I can see what got in the way.
39. As a user, I want every button disabled while an action runs, with "Putting back…" or "Moving…" in the status line, so that I can't start two at once.
40. As a user, I want an error to show in the status line, as it does today, so that a failure isn't silent.

### After Move

41. As a user, I want pressing Move on the Plan to take me straight to the Session I just ran, so that I don't have to go through a Summary screen to get there.
42. As a user, I want the Session to open with the run's result as its status line ("112 files moved · 0 skipped"), so that I see what the run just did.
43. As a user, I want any files the run skipped or that failed to be under Needs attention, so that problems from the run aren't lost now that Summary is gone.

### Unchanged behaviour

44. As a user, I want the Session rows on Home and History to stay as they are, with their Undo button, so that only this screen changes.
45. As a user, I want Sessions I ran before this change to open and work the same way, so that my History stays usable.

## Implementation Decisions

### Core (`src-tauri/core`)

- **The Journal's on-disk format does not change.** Its records, including the `reason` field of an outcome record, keep their names, so old and new Journals read the same way (ADR-0002). Nothing new is journaled: in particular, the Keeper is not recorded, so the Session screen does not explain why a file was planned.
- **Rename `reason` → `note`** on everything core returns above the Journal: the run/undo/redo outcome (`Outcome`), the Session's Operation (`SessionOperation`), and the internal journaled-operation state it is built from. Its doc stays "why it wasn't done" (skipped, failed, unrecoverable, or why the last Undo or Redo of it didn't happen). Mapping from the Journal record's `reason` to `note` happens where the Journal is read. The Plan's `Operation.reason` is a real Reason and is not renamed.
- **`SessionOperation` gains two name lists relative to the Scan Root**, like the Plan's `Operation.relative_path` / `destination`: `relative_path` (where the item was) and `relative_destination` (where the Operation put it, e.g. `Files Cleaner Trash/<Session ID>/Zoom/a.mp4`). They are derived from the journaled absolute paths and the Session's Scan Root when the Session is read. The UI must not rebuild them. Absolute `source` and `destination` stay, for Show in Finder.
- No new Tauri commands. `run_plan`, `undo`, `redo_operations`, `session_details` and `list_sessions` keep their shapes apart from the rename and the two new fields.

### UI (`src/`, plain ES modules, no framework, no build step: ADR-0003)

- **Session screen** is rewritten as described in *Solution*. Its stylesheet follows the Plan screen's (header, cards, list, sticky pane) and reuses its classes where they mean the same thing instead of copying them. The old per-row grid, state pills on rows and per-row buttons go.
- **Summary screen is deleted**, along with its navigation entry and its styles if nothing else uses them. The Plan screen's Move navigates to the Session screen with the Session ID and the run's result. The Session screen accepts an optional initial status message (shown as `describeSummary(result, "moved")`). The back button from a Session opened this way goes to Home.
- **New pure view module** for the Session screen (no DOM access), next to the Plan's view module:
  - Grouping by state: `moved` = `done`; `putBack` = `undone`; `attention` = `skipped`, `failed`, `unrecoverable`, `started`. Groups returned in that fixed order, empty ones left out.
  - The heading for a Session from its Operations and status: all moved → "{N} files moved to Files Cleaner Trash"; some put back → "{k} of {N} files put back"; all put back → "All {N} files are back"; stopped part-way → "Stopped after {k} of {N} files". "1 file" in the singular. Here N counts the Operations that were carried out.
  - Search, sort and paging: reuse the Plan view module's search/sort/paging/selection functions over `relativePath`. If their signatures assume Plan-only fields (`show` filter, `keeper`), generalise them minimally instead of duplicating them.
  - The pane sentence for a file, from its group: moved → "In {holding folder name}, inside {Scan Root name}."; put back → "Back in {folder}."; attention → a lead-in by state ("Skipped", "Failed", "Couldn't be recovered", "Stopped before this file finished") followed by `note` when there is one.
- **Selection:** the existing selection helpers (all / some / none and toggle over Operation IDs) drive the list's Select all box and the bulk action, as for the Plan's Include all.
- **Shared Session rows** (Home, History) keep their layout. `outcomes.js` keeps what Home and History use (`describeSummary`, `undoSections`) and reads `note` instead of `reason`. Remove whatever only Summary used.
- **Wording, exactly** (counts with `plural()`, sizes with `formatSize()`; the folder name from `relativeDestination[0]`; the Scan Root name from its last segment):
  - Reassurance: "Nothing was deleted. You can put back any file, or all of them, at any time."
  - Primary button: "Put all back". Busy text: "Putting back…" / "Moving…"
  - Cards: "{N} moved · {size} → {holding folder}, a folder inside {Scan Root name}"; "{N} put back · {size} → where they were"; "{N} need attention" with "Skipped, failed or stopped part-way. Nothing here was lost." ("1 needs attention")
  - List heading: "{N} moved, biggest first" (changing with the group and sort, like the Plan's)
  - Bulk action: "Put back {N} files" / "Move {N} files again"; with nothing ticked "Nothing ticked" (disabled)
  - Pane buttons: "Show in Finder", "Put back" / "Move again"
  - Status after an action: `describeSummary(result, "put back")` / `describeSummary(result, "moved again")`
  - Empty list: "No files match."
- **Accessibility:** every tick box has an accessible name ("Put back {file name}", "Select every file"). Cards are real buttons with `aria-pressed`. No tick box inside a `<button>`. The selected row is announced, and the existing focus ring is used.

### Visual direction: native and quiet, not AI-looking

Same rules as the Plan review spec: flat surfaces and hairline `--border` dividers; hierarchy from type size and weight; colour only where it means something (the accent for the primary button and the selected row; the state colour at most once, on the Needs attention card's count); no tinted icon tiles, glows, coloured card borders, drop shadows or hero headings; a plain "→" in text; existing tokens only; light and dark both work. The current card looks like a selected sidebar item. If something isn't covered here, choose the quieter option.

### Domain docs

- In `CONTEXT.md`, add a note under **Reason** that a skipped or failed Operation's explanation is not a Reason. Add a glossary term for it if a name is needed ("Note"), with _Avoid_: reason.
- Remove any mention of the Summary screen from `CONTEXT.md` and `README.md`.

## Testing Decisions

- Good tests check external behaviour through the highest seam: what a core function returns for a given Journal and filesystem, or what a pure view function returns for given Operations. They don't check private helpers or DOM structure.
- **Core seam: `tests/session.rs`** (existing integration tests through `run`, `undo`, `undo_operations`, `redo_operations`, `session_details`). Update the existing assertions from `reason` to `note`. Add assertions that `session_details` returns `relative_path` and `relative_destination` matching the Scan Root layout (holding folder / Session ID / relative path), including for a file at the top of the Scan Root. Add one test that a Journal written before this change (the existing record format with `reason`) still reads, with the text coming back as `note`.
- **UI seam: the new Session view module, with `node:test`**, following `tests/plan-view.test.js`. Cover grouping (each state into its group, fixed order, empty groups left out); each heading case, singular included; each pane sentence, with and without a `note`; search, sort and paging over Session Operations through the shared functions; selection fallback when search hides the selected file.
- Existing `plan-view` and `plan-selection` tests must keep passing after any generalisation.
- Commands: `cargo test` in `src-tauri/core`, `npm test` in `files-cleaner/`.
- Manual (by the developer, not the agent): open Sessions in each state (all moved, partly put back, all put back, stopped part-way, with skipped files) in light and dark; run a Plan and check it lands on the Session; keyboard review.

## Out of Scope

- Journaling the Keeper or the Plan's Reason, and showing why a file was planned.
- Changing the Session rows on Home and History.
- Changing the Journal format.
- Previews or thumbnails in the pane.
- Deleting Sessions or emptying the holding folder.
- The System Trash destination.

## Further Notes

- Prior art: `.scratch/plan-review/spec.md` and the Plan screen built from it. This screen should feel like the same app: same header shape, card-as-tab behaviour, list/pane split, right-click menu and paging.
- `describeSummary`'s "{N} items {done}" wording could become "files" to match. Fine to change if it stays consistent on Home and History.
- Do not commit: the developers make all commits.
