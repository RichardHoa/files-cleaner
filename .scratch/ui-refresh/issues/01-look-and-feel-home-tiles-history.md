# 01: Look and feel, Home tiles, History

**What to build:** The app gets its new look everywhere: near-black text, filled coloured pills for Session statuses, a colour and inline SVG icon per Operation kind, all colours as CSS custom properties, following the system's light or dark appearance. Home becomes a tiles screen: up to three Recent Scan Roots as clickable tiles that start a scan straight away (protected-path warning still applies), a "Pick folder" tile, the config as a small badge in the header, and only the latest three Sessions with a "See all" link. "See all" opens a new History screen listing every Session, ten per page, with numbered pages. Session rows (shared by Home and History) show date, Scan Root name, status pill, a count per Operation kind with its icon, total size, and Undo. The Scan results screen picks up the new styling without changing its layout. See `../spec.md`: user stories 1–30, 54–59.

**Blocked by:** None (can start immediately)

**Status:** resolved

### Styling base
- [x] Every colour (neutrals, accent, one per Operation kind, one per Session status, Operation states) is a CSS custom property; a dark set applies under the system's dark appearance
- [x] Operation kinds: Trash red, Move blue, Rename violet, Create Folder green, each with an inline SVG icon from one shared helper (no new dependencies, ADR-0003)
- [x] Session statuses as filled pills: Completed green, Undone amber, Partly undone striped amber, Stopped part-way red
- [x] Body text near-black; grey only for secondary details (paths, dates)
- [x] Scan results restyled with the new tokens and icons; its layout is unchanged

### Recent Scan Roots
- [x] The core records a Scan Root when its scan starts (after any protected-path warning is accepted), moves it to the front, removes repeats and keeps the newest three; stored in the app data folder by the core, not in webview storage
- [x] The core lists them newest first, each with when it was last scanned and whether the folder still exists, and can remove one; a missing or unreadable store gives an empty list
- [x] Core integration tests cover: ordering, no repeats, cap of three, missing folder flagged, remove, corrupt or missing store, survives reopening
- [x] Home shows them as tiles: folder name large, parent path small, "N days ago", full path in a tooltip; clicking scans that folder; × on hover removes it; a missing folder is greyed out and not clickable
- [x] A "Pick folder" tile next to them keeps the native picker; a friendly empty state when there are none

### Home
- [x] The header shows "Files Cleaner" and a config badge with the Rule count; clicking opens the config; a config error shows on the badge; it refreshes on window focus as today
- [x] Only the latest three Sessions are shown, with a "See all" link to History, and a friendly empty state when there are none

### Paged Session list
- [x] Listing Sessions takes an offset and a limit and returns the total plus that page, newest first; only the Journals on the requested page are read; unreadable Journals are still left out
- [x] Each Session summary carries the total size of its carried-out Operations and a count per Operation kind
- [x] Core integration tests cover: total, newest-first paging, partial last page, offset past the end, unreadable Journals skipped, per-kind counts and byte totals

### Session rows and History
- [x] One shared Session row component for Home and History: date, Scan Root folder name, status pill, per-kind counts with icons, total size, Undo (disabled when nothing is left to undo); clicking elsewhere opens the Session; an Undo's result shows above the list as today
- [x] History screen: ten Sessions per page, numbered page links with previous/next, total shown, Home button
- [x] Coming back from a Session opened from History returns to the same History page
- [x] `node:test` tests for the page-number helper (first, middle, last, single page, no Sessions) and the relative-time helper

### Verification
- [x] `cargo test` for the core and `npm test` pass
- [x] Checked by running the app in light and dark appearance (the frontend was checked in headless Chrome with stubbed Tauri commands, light and dark; the real window still needs a look)
