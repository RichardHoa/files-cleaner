# 01: Test trash and restore on all three OSes

**What to build:** A small throwaway program that answers whether the System Trash can keep the Undo promise on each OS. It creates its own temporary files and never works on a real folder. It sends them to the System Trash through a draft of each OS backend, reports where they landed, restores them, and prints the result of each case. The agent builds it and checks that it compiles; the developer runs it on each OS.

See `../spec.md`: *Implementation Decisions* (Gated rollout, the three backends).

**Blocked by:** none

**Status:** ready-for-human

Questions to answer, per OS:
- Does the OS report where the item landed? (Windows: is it available from `IFileOperation`'s progress sink?)
- Can the item be moved back out from that location, for a file, a folder and a Package? (macOS: does it need Full Disk Access? Windows: is moving it out a clean restore, or does it leave a record behind in the Recycle Bin?)
- What happens to two items with the same name?
- On a volume with no Trash (a network drive, or a removable drive), is that detected before anything happens, with the file left in place?

- [x] Program builds for macOS, Windows and Linux, with the exact run command in its README
- [x] Each question above has a printed pass or fail per OS
- [ ] **Developer:** run it on macOS, Windows and Linux, and record the results below
- [ ] Decision recorded: go ahead with ticket 02, or bring back any OS that can't restore reliably

## Comments

**Agent (2026-11-05):** Built at `prototypes/os-trash-test/`; the run commands are in its README. It has never been run, so nothing about any OS is known yet.

- **Compile checks:** `cargo build` and clippy pass on macOS. Windows and Linux pass `cargo check` and clippy from macOS, but haven't been linked, so the first `cargo run` on each is the first full build.
- **Printed results:** each question prints PASS or FAIL, with `????` when a run can't answer it. Q4 needs `--no-trash-volume <folder>` on a drive with no Trash, otherwise it prints NOT RUN.
- **Docs checked:** the FreeDesktop Trash spec (now at `specifications.freedesktop.org/trash/latest/`) and Microsoft's pages for the sink, `SHQueryRecycleBin` and the operation flags.

Open points for the run and for ticket 02:
- `TSF_DELETE_RECYCLE_IF_POSSIBLE` is documented only as "recycle on file delete, if possible", so it may not reveal a permanent delete. The test prints the flags on every delete. If ordinary recycles arrive without it, the sink doesn't refuse on it, and a no-Bin drive then shows the real outcome, which may destroy the test file.
- The Windows draft doesn't always refuse a permanent delete, on purpose, to learn this. Ticket 02 must always refuse (ADR 0001).
- On macOS, only `NSFeatureUnsupportedError` (3328) counts as "no Trash". That code is from memory of Foundation's headers and hasn't been checked against a real drive. Another code gives `????` and prints the code.
- A Terminal result on macOS is a hint about Full Disk Access, not proof for the signed app.
- Restore checks for a file at the destination and then renames, as `core` does today, so it can still overwrite in that gap. The spec says "without overwriting", so ticket 02 may want an atomic move.

**Developer run, macOS (2026-11-05, partial):**

- Q1 Landing: PASS. The OS reported a path in `~/.Trash` for every item.
- Q2 Move back: PASS for a file, a folder and a Package, with contents intact.
- Q2 Full Disk Access: NOT ANSWERED. This terminal could list `~/.Trash`, so the run can't show whether the app would need it.
- Q3 Same name: PASS. The second item landed as `same name.txt 10-40-01-877.txt`, and both came back with their own contents.
- Q4 No-Trash volume: NOT ANSWERED. The folder used, `~/Downloads`, has a Trash (`~/.Trash`), so it wasn't a no-Trash test. Needs a network share or similar.

Windows and Linux haven't been run yet.
