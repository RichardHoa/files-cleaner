# Plan review: what moves where, at a glance

Status: ready-for-agent

## Problem Statement

When I open the Plan I can't tell quickly which files will be moved, where they go, or why. The screen shows the Scan Root as two deep folder trees, Before and After, side by side. I have to read through every level of folders, each with its own tick box, to find the handful of files that are affected, then compare the two trees in my head to work out where each one ends up. The red "Trash" pill makes it look as if files are being deleted forever, when they actually move into a folder inside the Scan Root. The file that is *kept* (the whole reason the move is safe) appears only as small grey text. The totals line ("2 Operations ticked of 2 · Trash: 2 · 1.4 MB to trash") and the bare **Run** button use the app's internal words, not mine.

This gets worse as the Plan grows: with 100 duplicates the trees become a long scroll in which nothing stands out.

## Solution

The Plan screen is rebuilt around three questions, in this order: *how much is changing*, *where is it going*, and *why this particular file*.

1. **Header:** a large heading with the count ("112 files will move"), one reassuring line ("Nothing is deleted. Everything stays inside Downloads, and you can undo it from History."), and the two buttons, **Cancel** and **Move 112 files**.
2. **One card per Reason:** each card reads left to right as *what → where*: "100 duplicates · 1.5 GB → Files Cleaner Trash, a folder inside Downloads", with one plain sentence under it and an **Include all** tick box. The cards also act as tabs: clicking one shows that group's files below.
3. **List and detail, side by side:** on the left, the chosen group's files, one line each, biggest first, with search, sort, a Show All / Ticked / Unticked filter, and paging. On the right, a short detail pane for the selected file. It holds only two things: **why** this file moves ("An exact copy of a newer file, so this one goes to Files Cleaner Trash.") and, as a small note, **where the original is** ("Original: Downloads/Zoom/Untitled design 5262.mp4").

The Before/After trees are removed.

The layout was chosen from a clickable prototype (variant B, "Flow cards + detail pane"). See *Further Notes*. **The prototype's visual styling is not the target.** The developer judged it to look AI-generated. Keep the prototype's structure and wording, and restyle it as described in *Visual direction*.

## User Stories

### Seeing the whole Plan at once

1. As a user, I want the heading to say how many files will move, so that I know the size of the change before reading anything else.
2. As a user, I want to be told plainly that nothing is deleted and everything can be undone, so that I'm not afraid to press the button.
3. As a user, I want the main button to say what it does ("Move 112 files"), so that I don't have to guess what "Run" means.
4. As a user, I want the heading, the reassurance line and the button to update as I tick and untick, so that they always describe what will really happen.
5. As a user, I want the button disabled and labelled "Nothing ticked" when I have unticked everything, so that I can't start an empty Session.

### Where things go

6. As a user, I want one card per Reason showing how many files, how big they are in total, and the folder they go to, so that I can see where everything ends up without opening anything.
7. As a user, I want the destination named as a folder inside my Scan Root ("Files Cleaner Trash, a folder inside Downloads"), so that I understand the files are moved, not destroyed.
8. As a user, I want one plain sentence on each card saying why those files are included ("Exact copies. The newest copy of each file stays where it is."), so that I understand the group without reading a single file.
9. As a user, I want an "Include all" tick box on each card (ticked, unticked, or partly ticked), so that I can leave out a whole Reason in one click.
10. As a user, I want the card's count and size to reflect only the ticked files ("40 of 100 duplicates"), so that I can see how much I've left out.
11. As a user, I want clicking a card to show that group's files in the list, and the current card to be visibly marked, so that I know which list I'm looking at.

### The list

12. As a user, I want one line per file with a tick box, its name, the folder it is in, and its size, so that a long list stays easy to scan.
13. As a user, I want the list sorted biggest first by default, so that the files that matter most are at the top.
14. As a user, I want to sort by name or by folder instead, so that I can find a file another way.
15. As a user, I want to search by file name or folder, so that I can check a specific file.
16. As a user, I want to show only ticked or only unticked files, so that I can review what I've left out.
17. As a user, I want a file whose Duplicate Group has three or more copies to say "N copies", so that I understand why the same name appears more than once.
18. As a user, I want unticked files to stay in the list, visibly marked as left out, so that I can change my mind.
19. As a user, I want a long folder path to keep its end (the nearest folder) when it doesn't fit, with the full path on hover, so that I can tell folders apart.
20. As a user, I want the list to show 50 files at a time, with "Show 50 more" and "Show all N", so that a Plan of thousands of files stays fast.
21. As a user, I want "No files match." when a search or filter finds nothing, so that an empty list doesn't look broken.

### Why this file

22. As a user, I want clicking a file to show it in the detail pane, so that I can check one file without the list moving.
23. As a user, I want the pane to open on the first file in the list rather than empty, so that it's useful straight away.
24. As a user, I want the pane to tell me in one sentence why this file moves, so that I understand it without decoding paths or labels.
25. As a user, I want the pane to show where the original (the Keeper) is, so that I can trust that a copy is being kept.
26. As a user, I want a "Move this copy" tick box in the pane that stays in step with the file's tick box in the list, so that I can decide right where I read the reason.
27. As a user, I want to move through the list with the up and down arrow keys and tick with Space, so that I can review a big Plan from the keyboard.

### Unchanged behaviour

28. As a user, I want Cancel to take me back to the scan results, as it does today.
29. As a user, I want "Nothing to do: no duplicates were found." when the Plan is empty, as it is today.
30. As a user, I want Run to carry out only the ticked Operations and take me to the Session, as it does today.

## Implementation Decisions

### Core (`src-tauri/core`)

- `Operation` gains two fields, both relative to the Scan Root, as name lists like `relative_path`:
  - `destination: Vec<String>`: where the item will be once the Plan runs (for Trash: `Files Cleaner Trash/This Session/<relative_path>`). The UI must not rebuild this path itself.
  - `keeper: Option<Vec<String>>`: the Keeper's path, set for a duplicate's Trash Operation and `None` otherwise.
- `reason: String` stays: the Session and Summary screens still show it.
- The Journal is unaffected: it records its own fields, not the Plan's `Operation`, so existing Sessions stay readable.
- `Plan.before`, `Plan.after`, `TreeNode` and its re-export from `lib.rs` are removed. The Plan screen was their only consumer. Replace the tree assertions in `tests/plan.rs` with assertions on `destination` and `keeper`.

### UI (`src/`, plain ES modules, no framework, no build step: ADR-0003)

- `screens/plan.js` is rewritten. `styles/plan.css` is rewritten. The tree styles go.
- The UI groups Operations by Reason. Today the only group is Duplicates (Operations with a `keeper`), because Rules are parsed but not yet planned. Write the grouping so a Rule group can be added later without restructuring the screen: one card per group, each with a title, a one-line explanation, a destination and its own list.
- Put the pure logic in a new module (e.g. `plan-view.js`), unit-tested with `node:test`, with no DOM access: grouping, filtering (search is case-insensitive over the path from the Scan Root), sorting (size desc, then name; name; folder then size desc), paging, and the selected file falling back to the first visible file when filters hide it.
- `plan-selection.js`: keep `totals`. Replace `folderState` / `toggleFolder` (tree-based) with the same "all / some / none" and toggle logic over a plain list of Operation IDs, for the card's Include all.
- The "N copies" count comes from Operations sharing the same `keeper` (copies = Operations with that keeper + 1).
- For a file at the top of the Scan Root, the folder column shows the Scan Root's own name followed by `/` (e.g. `Downloads/`). The "Original:" note shows the Scan Root name followed by the Keeper's path.
- Wording, exactly (counts use `plural()`, sizes use `formatSize()`, the folder is the Scan Root's name):
  - Heading: "{N} files will move" / "1 file will move"; with nothing ticked: "Nothing will change"
  - Under it: "Nothing is deleted. Everything stays inside {Scan Root name}, and you can undo it from History."
  - Buttons: "Cancel" and "Move {N} files" / "Move 1 file"; with nothing ticked: "Nothing ticked" (disabled)
  - Duplicates card: "{N} duplicates" (or "{k} of {N} duplicates"), the ticked size, then "Files Cleaner Trash" with "a folder inside {Scan Root name}"; the sentence "Exact copies. The newest copy of each file stays where it is."; "Include all"
  - List heading: "{N} duplicates, biggest first", changing with the sort
  - Pane: the file name and size; "An exact copy of a newer file, so this one goes to Files Cleaner Trash."; "Original: {Scan Root name}/{keeper path}"; tick box "Move this copy"
  - Empty list: "No files match."
- Accessibility: every tick box has an accessible name ("Move {file name}", "Include every duplicate"). The cards are real buttons, or `role="button"` with keyboard activation. Do not nest a tick box inside a `<button>`: it is invalid, and the prototype hit this problem. The selected row is announced, and the existing focus ring is used.

### Visual direction: it must not look AI-generated

The developer rejected the prototype's look as "too AI". Match the rest of the app and native macOS apps (Finder, System Settings, Disk Utility), not a SaaS landing page. Concretely:

- **Avoid:** tinted rounded-square icon tiles; coloured top borders or glowing rings on cards; pastel-tinted panels; decorative arrow lines drawn across cards; an icon on every line; big bold "hero" headings; several accent colours competing; drop shadows on cards.
- **Do:** flat surfaces with hairline dividers (`--border`); hierarchy from type size and weight, not colour; colour only where it carries meaning, and sparingly (the Operation kind's colour for its one small icon, the accent for the primary button and the selected row); a plain "→" in text instead of a drawn arrow; the existing tokens in `styles/tokens.css` only, with no new raw colour values in components; light and dark appearance both checked.
- The current card should look selected the way a selected sidebar item or segmented control looks on macOS, not like a highlighted marketing tile.
- If a visual choice isn't covered here, choose the quieter option.

### Domain docs

- In `CONTEXT.md`, change the **Plan** entry's "shown as a before/after tree for the user to approve, untick, or cancel" to describe the new view: grouped by Reason, one line per Operation, with the selected Operation's Reason beside it.

## Testing Decisions

- Core: `cargo test`. `tests/plan.rs` checks that each duplicate's Trash Operation has `destination` = holding folder / This Session / its relative path, that `keeper` is its group's Keeper, and that Operation IDs stay stable between Plans (the existing test stays).
- UI logic: `npm test` (`node:test`) for the new view module and the updated selection helpers. Cover: grouping; search over name and folder; each sort; the three Show filters; paging (first page, "Show more", "Show all", fewer than one page); selection fallback when the selected file is filtered out; Include all states (all, some, none) and toggling; totals after unticking.
- Manual: run the app on a folder with a few duplicates and on one with 100+, in light and dark appearance; check keyboard review (arrows, Space, Tab), Cancel, Run, and the empty Plan.

## Out of Scope

- Planning Rules (Move / Rename by Rule). The screen only has to make room for a Rule group later.
- Choosing a different Keeper from the Plan screen.
- File previews or thumbnails in the detail pane.
- The System Trash destination (not supported yet).
- Restyling other screens.

## Further Notes

- **Prototype:** `files-cleaner/src/prototype-plan.html` and `prototype-plan.js`, run with `npm run prototype:plan` (or open the HTML file directly); use `?variant=B&size=large` for 112 fake files. Use it for layout, behaviour and wording only, not for styling. It is uncommitted. `src/` is the app's `frontendDist`, so the prototype would ship in the bundle: **delete both files and the `prototype:plan` script from `package.json` as part of this work.**
- This supersedes the Plan part of `.scratch/ui-refresh/` (the "By Reason" cards plus a tree tab): the tree tab is dropped.
- Do not commit: the developers make all commits.
