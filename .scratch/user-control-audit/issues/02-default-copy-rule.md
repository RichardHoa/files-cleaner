# 02: Default Copy Rule setting

**What to build:** A Settings option "Duplicates → Keep by default: Newest / Oldest / Shortest path", saved to the config, used from the next scan to pick which copy starts out kept.

**Blocked by:** 01

**Status:** resolved

- [x] Config holds the rule (default: newest), changeable from Settings only
- [x] Duplicate Groups are ordered by the rule
- [x] Test: with "oldest", the oldest copy is first

## Comments
- 2026-11-05: Done. `defaultCopyRule` is a top-level config key, as `duplicates` is taken by the old Duplicate Handling setting, which is still ignored. `copy_order` replaces `keeper_order` and `Found.kept` replaces `Found.keeper`. Ties: newest and oldest fall back on the shortest path, the shortest path falls back on the newest, then the first alphabetically. "Shortest path" counts the path's length in bytes, as the old tie-break did, not its folder depth.
