# 06: Port the built scan code to Windows and Linux

**What to build:** Ticket 01 was built for macOS only: `core/src/protection/` has just a `macos.rs` module, so the core doesn't compile on Windows or Linux, and some names and labels still say "Finder". Make the existing scan path work on all three desktop OSes, using the OS-agnostic vocabulary in `GLOSSARY.md` (System Folder, Disk Root, OS Metadata File, Package, Clash Suffix) and the per-OS lists in the spec (stories 5, 6, 10, 12, 13, and Implementation Decisions: Target OS, Letter case, Created date).

**Blocked by:** None (01 is resolved)

**Status:** resolved

- [x] `protection/` has a module per OS (`macos.rs`, `windows.rs`, `linux.rs`), selected with `#[cfg(target_os = ...)]`, each giving that OS's System Folders and Disk Roots (spec story 10)
- [x] Shared code applies on every OS: the OS Metadata File list (`.DS_Store`, `Icon\r`, `.localized`, `Thumbs.db`, `desktop.ini`, `.directory`), the app-library list, and the Package extension list; on macOS, Package detection still also asks the OS (`NSURLIsPackageKey`)
- [x] `FINDER_METADATA` becomes `OS_METADATA_FILES`, and reasons say "is an OS Metadata File", "is a System Folder", "is a Disk Root" instead of "Finder metadata", "system folder", "your Library folder"
- [x] Hidden entries: names starting with a dot on every OS, plus the Hidden or System attribute on Windows
- [x] Protected Path matching stays case-insensitive on every OS; the comment on `is_within` no longer says it mirrors macOS
- [x] A missing created date (e.g. on some Linux file systems) is handled: `created` is `None` and the Scan results show a dash
- [x] `load_config` rejects `trashDestination: "systemTrash"` with a clear "not supported yet" error (System Trash is deferred, ADR-0001); the config test that expects `SystemTrash` is updated
- [x] UI: `revealInFinder` becomes `revealInFileManager`, and the "Show in Finder" menu item becomes "Show in folder"
- [x] `~` expands to the user's home folder on every OS (`%USERPROFILE%` on Windows), and Never-Touch Paths and Rule targets accept Windows paths (`C:\…`)
- [x] Per-OS tests for System Folders, Disk Roots and Hidden detection, gated to their OS; shared-list tests run everywhere; `cargo test` passes on macOS, Windows and Linux (at least `cargo check --target` for the other two from a Mac)
- [x] `tauri.conf.json` bundle settings and icons work for Windows and Linux builds
- [ ] Manual check on each OS: pick a folder, see the scan table and skipped count, "Show in folder" opens the file manager

## Comments

**Parked (2026-10-16).** Content grouping may use macOS-only frameworks (Vision OCR, `NLEmbedding`, Foundation Models) behind a swappable interface. Porting now risks doing the work twice. Pick this up once the content-grouping prototype has settled that choice and it is recorded in an ADR.

**Unparked (2026-10-20).** Grouping will use only cross-platform tools (see `.scratch/basic-pipeline/map.md`), so the platform decision no longer blocks this. Do it early, before more code builds on the macOS-only core.

**2026-10-20, implemented.**
- `protection/{macos,linux,windows}.rs` each give `is_disk_root`, `builtin_reason` (System Folders), `is_hidden` (Windows: Hidden or System attribute) and `is_package` (only macOS asks the OS). Linux Disk Roots: `/`, `/mnt[/<disk>]`, `/media[/<user>[/<disk>]]`, `/run/media/<user>/<disk>`. `refusal` stops walking up at a Disk Root, so a disk under `/run/media` isn't "inside `/run`". Windows paths come back as `C:\…`, not `\\?\C:\…`, and drive letters compare in either form.
- Checked: `cargo test` on Linux (as root, and as a normal user for the unreadable-folder tests); `cargo check --tests` for `x86_64-pc-windows-msvc` and `x86_64-apple-darwin`. The Windows- and macOS-only tests have only been compiled, not run.
- Manual check on Linux (Xvfb): scan table, skipped count and the "Show in folder" item work. "Show in folder" itself needs a desktop with a file manager (the container had no D-Bus portal, and the app showed the error). Still to check by hand: macOS, Windows, and "Show in folder" on a Linux desktop.
