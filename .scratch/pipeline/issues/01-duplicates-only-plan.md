# 01: The duplicate step: a Plan of duplicates only

**What to build:** The Plan built after a scan proposes only Duplicates: every non-Keeper copy in a Duplicate Group, trashed. Rules are frozen: the code and its tests stay, but no Plan contains Rule Operations and the config's `rules` are ignored. The Duplicate Handling setting goes, because running the duplicate step (or skipping it) is the choice now.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `build_plan` proposes Trash Operations for non-Keeper copies only, with Reason Duplicate; it never matches Rules
- [x] `duplicates` is removed from `Config` and `DuplicateHandling` is deleted; a config that still has a `duplicates` key loads without error (the key is ignored)
- [x] The Rules code (`rules.rs`, Rule parsing in `config.rs`) and its tests stay and pass; tests that build Rule Plans through `build_plan` are deleted or rewritten to call the Rules code directly
- [x] `core/tests/plan.rs`: the `leave`/`followRules` test goes; the duplicate tests still pass
- [x] Home's config badge no longer shows a Rule count (it shows the config is fine, or its error)
- [x] The example config written on first launch has no `rules` or `duplicates` keys
- [x] `cargo test` and `npm test` pass

## Comments

`build_plan` now proposes only Trash for non-Keeper copies (Reason Duplicate), plus the folders that empties, unticked. The Rule planning moved to `build_rule_plan`, which no caller in the app uses; `tests/rules.rs`, `tests/rule_sessions.rs` and the Rule cases in `tests/safety.rs` call it directly, so the Move / Create Folder executor paths stay tested. `Config.duplicates` and `DuplicateHandling` are gone; an old `duplicates` key (any value) is ignored. The example config keeps only `trashDestination` and `neverTouch`. Home's badge reads "Config OK" or "Config error".
