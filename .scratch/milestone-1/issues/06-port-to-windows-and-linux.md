# 06: Port the built scan code to Windows and Linux

**What to build:** Ticket 01 was built for macOS only: `core/src/protection/` has just a `macos.rs` module, so the core doesn't compile on Windows or Linux, and some names and labels still say "Finder". Make the existing scan path work on all three desktop OSes, using the OS-agnostic vocabulary in `CONTEXT.md` (System Folder, Disk Root, OS Metadata File, Package, Clash Suffix) and the per-OS lists in the spec (stories 5, 6, 10, 12, 13, and Implementation Decisions: Target OS, Letter case, Created date).

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] `protection/` has a module per OS (`macos.rs`, `windows.rs`, `linux.rs`), selected with `#[cfg(target_os = ...)]`, each giving that OS's System Folders and Disk Roots (spec story 10)
- [ ] Shared code applies on every OS: the OS Metadata File list (`.DS_Store`, `Icon\r`, `.localized`, `Thumbs.db`, `desktop.ini`, `.directory`), the app-library list, and the Package extension list; on macOS, Package detection still also asks the OS (`NSURLIsPackageKey`)
- [ ] `FINDER_METADATA` becomes `OS_METADATA_FILES`, and reasons say "is an OS Metadata File", "is a System Folder", "is a Disk Root" instead of "Finder metadata", "system folder", "your Library folder"
- [ ] Hidden entries: names starting with a dot on every OS, plus the Hidden or System attribute on Windows
- [ ] Protected Path matching stays case-insensitive on every OS; the comment on `is_within` no longer says it mirrors macOS
- [ ] A missing created date (e.g. on some Linux file systems) is handled: `created` is `None` and the Scan results show a dash
- [ ] `load_config` rejects `trashDestination: "systemTrash"` with a clear "not supported yet" error (System Trash is deferred, ADR-0001); the config test that expects `SystemTrash` is updated
- [ ] UI: `revealInFinder` becomes `revealInFileManager`, and the "Show in Finder" menu item becomes "Show in folder"
- [ ] `~` expands to the user's home folder on every OS (`%USERPROFILE%` on Windows), and Never-Touch Paths and Rule targets accept Windows paths (`C:\…`)
- [ ] Per-OS tests for System Folders, Disk Roots and Hidden detection, gated to their OS; shared-list tests run everywhere; `cargo test` passes on macOS, Windows and Linux (at least `cargo check --target` for the other two from a Mac)
- [ ] `tauri.conf.json` bundle settings and icons work for Windows and Linux builds
- [ ] Manual check on each OS: pick a folder, see the scan table and skipped count, "Show in folder" opens the file manager
