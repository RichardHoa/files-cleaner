---
status: accepted
---

# Plain JS modules for the UI; all logic in a Rust core

The frontend is plain ES modules served directly by Tauri, with no framework and no build step. All scanning, hashing, planning, execution, journaling and Undo live in a Rust core library with no Tauri dependency, behind thin Tauri commands. We chose this to keep dependencies minimal and to make the safety-critical code testable without a UI.

## Considered Options

- **Svelte + Vite:** rejected as unnecessary weight for a handful of screens. Don't "upgrade" the UI to a framework without revisiting this decision.

## Consequences

- No UI libraries either: icons are inline SVG, drawn by hand.
- The core's public functions are the test seam; the Tauri commands only find paths and forward calls.
