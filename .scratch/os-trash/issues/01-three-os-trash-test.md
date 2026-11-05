# 01: Test trash and restore on all three OSes

**What to build:** A small throwaway program that answers whether the System Trash can keep the Undo promise on each OS. It creates its own temporary files and never works on a real folder. It sends them to the System Trash through a draft of each OS backend, reports where they landed, restores them, and prints the result of each case. The agent builds it and checks that it compiles; the developer runs it on each OS.

See `../spec.md`: *Implementation Decisions* (Gated rollout, the three backends).

**Blocked by:** none

**Status:** ready-for-agent

Questions to answer, per OS:
- Does the OS report where the item landed? (Windows: is it available from `IFileOperation`'s progress sink?)
- Can the item be moved back out from that location, for a file, a folder and a Package? (macOS: does it need Full Disk Access? Windows: is moving it out a clean restore, or does it leave a record behind in the Recycle Bin?)
- What happens to two items with the same name?
- On a volume with no Trash (a network drive, or a removable drive), is that detected before anything happens, with the file left in place?

- [ ] Program builds for macOS, Windows and Linux, with the exact run command in its README
- [ ] Each question above has a printed pass or fail per OS
- [ ] **Developer:** run it on macOS, Windows and Linux, and record the results below
- [ ] Decision recorded: go ahead with ticket 02, or bring back any OS that can't restore reliably

## Comments
