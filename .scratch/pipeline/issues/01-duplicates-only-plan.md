# 01: The duplicate step: a Plan of duplicates only

**What to build:** The Plan built after a scan proposes only Duplicates: every non-Keeper copy in a Duplicate Group, trashed. Rules are frozen: the code and its tests stay, but no Plan contains Rule Operations and the config's `rules` are ignored. The Duplicate Handling setting goes, because running the duplicate step (or skipping it) is the choice now.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `build_plan` proposes Trash Operations for non-Keeper copies only, with Reason Duplicate; it never matches Rules
- [ ] `duplicates` is removed from `Config` and `DuplicateHandling` is deleted; a config that still has a `duplicates` key loads without error (the key is ignored)
- [ ] The Rules code (`rules.rs`, Rule parsing in `config.rs`) and its tests stay and pass; tests that build Rule Plans through `build_plan` are deleted or rewritten to call the Rules code directly
- [ ] `core/tests/plan.rs`: the `leave`/`followRules` test goes; the duplicate tests still pass
- [ ] Home's config badge no longer shows a Rule count (it shows the config is fine, or its error)
- [ ] The example config written on first launch has no `rules` or `duplicates` keys
- [ ] `cargo test` and `npm test` pass
