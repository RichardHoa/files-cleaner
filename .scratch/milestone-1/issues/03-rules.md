# 03: Rules: trash, move, rename, and Duplicate Handling

**What to build:** The user's Rules from the config now shape the Plan. Rules match files and Packages by extension and/or name glob (both must match when both are given), and the first matching Rule wins. A Rule can trash a file, or move it to a folder (absolute, `~/`, or relative to the Scan Root) with an optional rename. Missing target folders show up as Create Folder Operations. The Duplicate Handling setting decides whether non-Keeper copies are trashed, treated by Rules like any other file, or left alone. Everything runs and undoes through the Session flow from ticket 02.

**Blocked by:** 02

**Status:** resolved

- [x] A small hand-written glob matcher (no glob crate) plus an extension matcher; first match wins; Rules never match folders
- [x] `trash` Rules produce Trash Operations; `moveTo` Rules produce Move Operations, whose destination carries the new name when `rename` is set
- [x] Rename placeholders `{name}`, `{ext}` and `{modified}` (YYYY-MM-DD, with no date crate)
- [x] Create Folder Operations are added for missing targets and appear in the tree; Undo removes them only if they're empty
- [x] Name clashes, both with existing files and between items in the same Plan, get a Clash Suffix (`notes (1).pptx`), with names differing only in letter case counting as clashes on every OS, and the suffix is recalculated at run time if the destination was taken after planning; nothing is ever overwritten
- [x] Duplicate Handling `trash` / `followRules` / `leave` behaves as described in the spec, with `trash` as the default
- [x] Undo restores moved and renamed files to their original names and locations, and skips instead of overwriting when the original location is now taken
- [x] Tests cover precedence, every Duplicate Handling mode, rename output, and relative, `~` and absolute targets

## Comments

**Implementation (2026-10-16), uncommitted.** Open decision: the `moveTo` + `rename` checkbox is left unticked.

- `moveTo` with `rename` produces **one** Move Operation whose destination has the new name. There is no separate Rename Operation, so a rename can't be ticked apart from its move. That contradicts the "(plus Rename …)" wording above, story 35 and GLOSSARY.md. Decide whether to split them, or update the spec and glossary.
- A **relative** `moveTo` that lands on a Protected Path or Package below the Scan Root (e.g. `Files Cleaner Trash/x`, `proj/node_modules`) makes that Rule propose nothing, and the user is not told. Absolute and `~` targets are still rejected at config load. This may belong with ticket 04's non-tickable error items.
- Other readings:
  - A file already in its Rule's folder is left alone, so a rename template isn't applied again on every Plan.
  - `{name}` is the name without its extension. The original extension is always kept, so `{name}.{ext}` gives `a.pdf.pdf`.
  - `{modified}` is the UTC date.
  - The glob supports only `*` and `?`, and ignores letter case.
  - Config rejects rename templates that are empty, contain `/` or `\`, or use an unknown placeholder.
- An unticked Create Folder makes the Moves into that folder get skipped, with the Note "The folder … doesn't exist". Undo removes a created folder only if it is truly empty, so even a `.DS_Store` keeps it.
- Redo doesn't recalculate Clash Suffixes. It is skipped if the journaled destination is taken.

**Decision (2026-10-16): resolved.**

- `moveTo` + `rename` stays **one** Move Operation whose destination has the new name, as built. Rename is not a separate Operation. The spec (story 35) and `GLOSSARY.md` (**Operation**) were updated to match.
- The relative `moveTo` that lands on a Protected Path or Package and silently proposes nothing moved to ticket 04, as a "blocked target" error item.
