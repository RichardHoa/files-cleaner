# 03: Rules: trash, move, rename, and Duplicate Handling

**What to build:** The user's Rules from the config now shape the Plan. Rules match files and Packages by extension and/or name glob (both must match when both are given), and the first matching Rule wins. A Rule can trash a file, or move it to a folder (absolute, `~/`, or relative to the Scan Root) with an optional rename. Missing target folders show up as Create Folder Operations. The Duplicate Handling setting decides whether non-Keeper copies are trashed, treated by Rules like any other file, or left alone. Everything runs and undoes through the Session flow from ticket 02.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] A small hand-written glob matcher (no glob crate) plus an extension matcher; first match wins; Rules never match folders
- [ ] `trash` Rules produce Trash Operations; `moveTo` Rules produce Move Operations (plus Rename when `rename` is set)
- [ ] Rename placeholders `{name}`, `{ext}` and `{modified}` (YYYY-MM-DD, with no date crate)
- [ ] Create Folder Operations are added for missing targets and appear in the tree; Undo removes them only if they're empty
- [ ] Name clashes, both with existing files and between items in the same Plan, get Finder-style suffixes (`notes (1).pptx`), and the suffix is recalculated at run time if the destination was taken after planning; nothing is ever overwritten
- [ ] Duplicate Handling `trash` / `followRules` / `leave` behaves as described in the spec, with `trash` as the default
- [ ] Undo restores moved and renamed files to their original names and locations, and skips instead of overwriting when the original location is now taken
- [ ] Tests cover precedence, every Duplicate Handling mode, rename output, and relative, `~` and absolute targets
