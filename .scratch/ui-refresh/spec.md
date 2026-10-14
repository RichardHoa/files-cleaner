# UI refresh: recent folders, Session history, visual Plan and Session screens

Status: ready-for-agent

## Problem Statement

The app works, but it is hard to read at a glance. Almost everything is grey text: the Session screen and the Plan screen tell me what happened or will happen only in sentences and small grey tags, so I can't see at a glance how much is being trashed, which copy of a duplicate is kept, or which changes are undone. The Home screen lists every Session I have ever run, so it will grow without end. And every time I want to clean the same folder again (usually Downloads), I have to go through the native folder picker again.

## Solution

A visual refresh across the app, plus two navigation improvements.

- **Home** becomes a tiles screen: my last three Recent Scan Roots as large tiles I can click to scan again, plus a "Pick folder" tile; the config shrinks to a small badge in the header; below, only the latest three Sessions, with a "See all" link.
- A new **History** screen lists every Session, ten per page, with numbered pages.
- **Session detail** shows stat cards (how many trashed, how much moved to Trash, how many undone), a done/undone progress bar, and Operations grouped by kind, each showing where the item was and where it is now. After Run, the app lands here directly.
- **Plan** opens on a "By Reason" view: one card for duplicates (each Duplicate Group shows its Keeper, marked as kept, with its copies below) and one card per Rule. The before/after tree stays on a second tab, with removed items struck through in red on the Before side and new items in green on the After side.
- Throughout: each Operation kind and each Session status has its own colour and icon, body text is near-black, and the app follows the system's light or dark appearance.

## User Stories

### Recent Scan Roots

1. As a user, I want my last three Recent Scan Roots shown as tiles on Home, so that I can clean a folder I clean often with one click.
2. As a user, I want the most recently scanned folder first, so that the one I most likely want is easiest to reach.
3. As a user, I want a folder to appear only once in the tiles even if I scanned it many times, so that the three tiles show three different folders.
4. As a user, I want a folder to count as recent only once its scan has actually started, so that a folder I backed out of at the protected-path warning doesn't push out a useful one.
5. As a user, I want each tile to show the folder's name large, its parent path small, and how long ago I last scanned it ("2 days ago"), so that I can tell similar folders apart.
6. As a user, I want the full path in a tooltip when it doesn't fit, so that long paths don't break the tile.
7. As a user, I want clicking a tile to start a scan of that folder right away, so that I skip the folder picker.
8. As a user, I want the protected-path warning to still appear when a tile's folder is a Protected Path, so that the shortcut is exactly as safe as picking.
9. As a user, I want a tile whose folder no longer exists (deleted, renamed, or on an unplugged drive) to show greyed out and not start a scan, so that I understand why it can't be used.
10. As a user, I want a × on hover to remove any tile, so that I can clear folders I no longer care about.
11. As a user, I want a "Pick folder" tile next to the recent ones, so that choosing a new folder is still one click away.
12. As a user, I want the Recent Scan Roots remembered after I quit and reopen the app, so that the shortcut works across days.

### Home

13. As a user, I want the config shown as a small badge with the Rule count in the header, so that its long path doesn't clutter Home.
14. As a user, I want clicking the config badge to open the config file, so that editing Rules is still one click.
15. As a user, I want the badge to show when the config has an error, so that I notice a broken config before scanning.
16. As a user, I want Home to show only the latest three Sessions, so that Home stays short however many Sessions I run.
17. As a user, I want a "See all" link next to the Sessions, so that I can reach older ones.
18. As a user, I want a friendly empty state when I have no Sessions or no Recent Scan Roots yet, so that a fresh install doesn't look broken.

### Session rows (Home and History)

19. As a user, I want each Session row to show its date, the Scan Root's folder name, a coloured status pill, a count per Operation kind with its icon, and the total size, so that I can tell Sessions apart without opening them.
20. As a user, I want each status in its own colour (Completed green, Undone amber, Partly undone striped amber, Stopped part-way red), so that I can spot Sessions that need attention.
21. As a user, I want an Undo button on each row, disabled when there is nothing left to undo, so that I can reverse a Session without opening it.
22. As a user, I want clicking anywhere else on the row to open that Session, so that rows are easy to hit.
23. As a user, I want the result of an Undo shown above the list, as it is today, so that I know what was restored or skipped.

### History

24. As a user, I want a History screen with every Session, newest first, so that I can find and undo an old one.
25. As a user, I want ten Sessions per page, so that each page is quick to scan.
26. As a user, I want numbered page links with previous and next, so that I can jump straight to older Sessions.
27. As a user, I want the total number of Sessions shown, so that I know how far back my history goes.
28. As a user, I want to come back to the same History page after opening a Session, so that I don't lose my place.
29. As a user, I want History to stay fast with hundreds of Sessions, so that a long history doesn't slow the app down.
30. As a user, I want a Home button on History, so that I can get back.

### Session detail

31. As a user, I want stat cards at the top (Operations by kind, size moved to Trash, how many undone), so that I see what the Session did at a glance.
32. As a user, I want the size labelled "moved to Trash", not "freed", so that I'm not misled: files in the Holding Folder still take up disk space.
33. As a user, I want a progress bar showing how many Operations are done versus undone, so that I can see a partly undone Session's state.
34. As a user, I want the Scan Root shown with a folder icon and the status pill in the header, so that I know which Session I'm looking at.
35. As a user, I want Operations grouped by kind under a heading with a count ("Trashed (12)"), so that a long Session stays readable.
36. As a user, I want each Operation row to show its icon, file name, size, where it was and where it is now with an arrow between them, so that I understand exactly what happened to each file.
37. As a user, I want each row's state (done, undone, skipped) shown in colour, so that I can see which files are back.
38. As a user, I want Show in Finder and Undo or Redo on each row, as today, so that I keep the controls I have now.
39. As a user, I want Undo all and Redo all near the stat cards, so that whole-Session actions are easy to find.
40. As a user, I want the back button to return me to History if I came from History, and to Home otherwise, so that navigation feels natural.
41. As a user, I want to land on the Session detail with a "Done ✓" banner right after I press Run, so that I see the result in the same visual form and can undo straight away.

### Plan

42. As a user, I want a summary bar at the top showing how many Operations are ticked and the size by kind, so that I know the scale of the change before running it.
43. As a user, I want the Plan to open on a "By Reason" tab, so that I see why each file is affected.
44. As a user, I want one card for duplicates, titled with how many files and how much space, so that duplicate cleanup is in one place.
45. As a user, I want each Duplicate Group in that card to show its Keeper marked "kept ✓" with the copies to trash below it, so that I'm confident the right copy stays.
46. As a user, I want one card per Rule, titled with the Rule's name, so that I can see what each of my Rules did.
47. As a user, I want a tick box on every Operation and a "tick all" on every card and every Duplicate Group, so that I can approve or skip a whole group at once.
48. As a user, I want the summary bar and Run button to follow my ticks immediately, so that I always know what Run will do.
49. As a user, I want a "Tree" tab with the before/after trees side by side, so that I can still review the change folder by folder.
50. As a user, I want items being removed shown struck through in red on the Before side and items being added in green on the After side, so that the difference jumps out.
51. As a user, I want chains of folders that each hold only one folder collapsed into one row (`Session_6_export/hello/ani/`), so that deep nesting doesn't push the files off screen.
52. As a user, I want ticking in either tab to stay in sync with the other, so that switching tabs never changes what Run will do.
53. As a user, I want Cancel to take me back to the Scan results, as today, so that nothing changes until I press Run.

### Scan results

54. As a user, I want the Scan results screen to use the same colours, icons and darker text, so that the app feels consistent, without its layout changing.

### Look and feel

55. As a user, I want each Operation kind to have its own colour and icon (Trash red, Move blue, Rename violet, Create Folder green) everywhere it appears, so that I learn them once.
56. As a user, I want status and kind tags as filled, coloured pills rather than grey outlines, so that they're easy to read.
57. As a user, I want body text near-black, with grey only for secondary details like paths and dates, so that the screens are easy to read.
58. As a user, I want the app to follow my system's light or dark appearance, so that it doesn't glare at night.
59. As a user, I want Trash shown in red but always worded as reversible ("moved to Trash"), so that the colour doesn't make me think files are deleted.

## Implementation Decisions

### Core (Rust)

- **Recent Scan Roots** live in a new core module. Its interface: record a Scan Root (it moves to the front, repeats are removed, only the newest three are kept); list them, newest first, each with when it was last scanned and whether the folder still exists; remove one. It is stored as a small file in the app data folder, next to the Journals and the config, written by the core, not by the webview's storage. A missing or unreadable file means an empty list, never an error that blocks Home.
- A Scan Root is recorded when its scan starts, after any protected-path warning has been accepted, not when the folder is picked.
- **Paged Session list**: listing Sessions takes an offset and a limit and returns the total count plus that page's Sessions, newest first. Journal ids sort by start time, so the core sorts ids by name and reads only the Journals on the requested page. Journals that can't be understood are still left out, as today.
- Each Session summary gains the **total size** of its carried-out Operations and a **count per Operation kind**, so rows can show "🗑 3 · 12.4 MB" without loading details.
- **Structured Reason**: each planned Operation carries a Reason instead of free text: either Duplicate (with the Keeper's path relative to the Scan Root, and an id for its Duplicate Group) or Rule (with the Rule's name). The UI writes the sentence. Today only Duplicate exists in practice; the Rule variant is in place for when Rules produce Operations.
- The Tauri commands stay thin wrappers that mirror the core (ADR-0003): listing Sessions gains offset and limit; new commands list, record and remove Recent Scan Roots, or recording happens inside the existing scan command when the scan starts.

### UI (plain JS, ADR-0003)

- **Styling base**: every colour is a CSS custom property on the root: neutrals, accent, one colour per Operation kind, one per Session status, and Operation states. A dark set is applied under the system's dark appearance. Icons are inline SVG built by a small shared helper. No new dependencies.
- **Pure helpers, kept out of the screens so they can be tested:**
  - grouping a Plan's Operations into Reason cards (one Duplicates card with its Duplicate Groups, Keeper first; one card per Rule), with counts and sizes per card and per group;
  - collapsing single-child folder chains in a tree;
  - the page numbers to show for History, given the current page and the total;
  - relative times such as "2 days ago";
  - the existing tick logic, extended so that ticking a card or a Duplicate Group works the same way as ticking a folder, and both tabs share one set of ticked ids.
- **Screens:**
  - Home is rebuilt as header + tiles + latest-three Sessions.
  - History is a new screen.
  - Session detail is rebuilt.
  - Plan is rebuilt with the two tabs.
  - Scan results is restyled only.
  - The separate post-Run summary screen is removed: Run goes straight to Session detail with a "Done ✓" banner.
  - Session rows are one shared component used by Home and History.
- **Navigation**: the Session screen remembers whether it was opened from History (and which page) or from Home, and its back button returns there.

## Testing Decisions

- A good test checks behaviour through a public interface (what goes in, what comes out, what's on disk) and never internals, so the code underneath can be refactored freely.
- **Core seam**: integration tests in the core's test folder against temp directories, as the existing session and plan tests do:
  - Recent Scan Roots: recording moves a folder to the front, removes repeats, keeps three; a deleted folder is listed as missing; remove works; a missing or corrupt store gives an empty list; the list survives reopening (it reads from disk).
  - Paged Session list: total count is right; pages are newest first, ten at a time; the last page is partial; an offset past the end is empty; unreadable Journals are skipped without shifting other pages wrongly; per-kind counts and byte totals match the Session's carried-out Operations.
  - Structured Reason: a duplicate's Operation names its Keeper and Duplicate Group; copies of the same group share the group id.
- **JS seam**: `node:test` unit tests in the frontend test folder, as the existing plan-selection and format tests do, for each pure helper listed above: grouping by Reason, collapsing folder chains, page numbers (first, middle, last page, a single page, no Sessions), relative times, and ticking cards or groups.
- Screens are not tested automatically; they stay thin and are checked by running the app in both light and dark appearance.

## Out of Scope

- Filters or search on History (by Scan Root or status).
- Redesigning the Scan results layout, beyond colours, icons and text.
- Rules producing Move, Rename or Create Folder Operations: this spec only makes room for them (colours, icons, Rule cards).
- The System Trash as a Trash Destination.
- Pinning Recent Scan Roots, or keeping more than three.
- A light/dark switch inside the app; only following the system.

## Further Notes

- The new terms **Recent Scan Root** and **Reason** are in `CONTEXT.md`.
- The layouts were chosen during grilling: Home as tiles, Session detail as stat cards and a grouped list, Plan grouped by Reason with the tree on a tab.
- The "moved to Trash" wording is deliberate: with the Holding Folder as Trash Destination, nothing leaves the disk.
