# 03: Delete all Sessions

**What to build:** **Delete all Sessions** on the Settings screen. It clears History and the Journals and touches no other file. It asks first: "Delete all 12 Sessions? You won't be able to undo them from the app. Your files won't change." It includes interrupted Sessions, and it's unavailable with no Sessions or during a Run or Undo.

See `../spec.md`: *Implementation Decisions* (Delete all Sessions); user stories 28–34.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] `core` entry point takes the Sessions lock and removes every Journal, reporting Busy during a Run or Undo
- [ ] Tauri command, and a button on Settings with the confirmation; no control on any other screen
- [ ] History shows no Sessions afterwards
- [ ] Test: after a Run, deleting all Sessions empties the list, and the moved files stay where they are

## Comments
