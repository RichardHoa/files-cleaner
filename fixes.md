# Fixes from the code quality review of `7149fc8`

Ticked once done and verified (`cargo test`, `npm test`).

## Core (Rust)

- [x] **1. Journal records as phase × result.** Replace the 15-variant `Record` (`Done`/`Redone`/`Restored`, `Skipped`/`UndoSkipped`/`RedoSkipped`, `Intent`/`UndoIntent`/`RedoIntent`, …) with `Intent { operation_id, phase, … }` + `Outcome { operation_id, phase, result, reason }`, and fold them with one pure `next_state` function.
- [x] **2. One journaled-apply loop.** `run`, `undo_selected` and `redo_operations` share one loop (intent → attempt → outcome record → outcome); one `Outcome` constructor.
- [x] **3. Delete phantom backward compatibility.** `planned` is required (no `#[serde(default)]`, no "recorded before Redo existed" branch, `size` not optional); `Operation` holds a flattened `SourceState` instead of repeating its fields.
- [x] **4. Session IDs in one place.** Generate, validate and order Session IDs in one module; zero-pad the same-millisecond suffix so a plain sort is start order (`start_order` goes).
- [x] **5. Typed Journal header.** `journal::read` returns the Session header separately, so `unreachable!` goes.
- [x] **6. Use or drop write-only records.** Drop `UndoStarted`/`UndoFinished`; an intent with no outcome (a crash mid-Undo/Redo) is resolved by checking where the item actually is, instead of being marked Unrecoverable.

## Tauri layer

- [x] **7a. `blocking` helper** for the repeated `spawn_blocking(..).await.map_err(..)?.map_err(..)`.
- [x] **7b. One Undo command** taking optional Operation IDs, instead of `undo` + `undo_operations`.
- [x] **7c. Atomic latest scan + Plan.** One mutex; a new scan drops the old Plan.

## UI

- [x] **8a. `busy` helper** for the disable-buttons / status / try / describeError pattern.
- [x] **8b. Move `undoSections`/`outcomeSection`** out of `screens/summary.js` into a shared module.
- [x] **8c. One description of an Undo result** instead of `describeUndo` and the `undoSections` lead line.

## Later

- [x] **9. Split `styles.css`** into `src/styles/` (tokens, base, components and one file per screen), linked in the original cascade order.

## Behaviour changes to know about

- **Journal format changed.** Journals written by `7149fc8` can't be read any more (it never shipped, so no compatibility layer). Delete `sessions/` in the app data folder if you have test Sessions from that build.
- **Session IDs** for Sessions started in the same millisecond end in `-002`, `-003`, … instead of `-2`, `-3`.
- **Crash recovery is new:** an Undo, Redo or Run cut short after its move is journaled as done on the next Undo or Redo, instead of being marked Unrecoverable or left stranded. 4 new tests in `core/tests/session.rs`.
- **Summary text:** "N items trashed · 0 skipped" instead of "N done · 0 skipped · 0 failed"; failed and unrecoverable counts show only when there are some.
- **Session screen:** an Undo or Redo that errors shows the error in the status line without reloading the Session, where before it reloaded.
