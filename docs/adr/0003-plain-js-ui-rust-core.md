# Plain JS modules for the UI; all logic in a Rust core

The frontend is plain ES modules served directly by Tauri, with no framework and no build step; all scanning, hashing, rule matching, planning, execution, journaling and Undo live in Rust behind thin Tauri commands. We chose this to keep dependencies minimal and to make the safety-critical code testable without a UI. Svelte + Vite was considered and rejected as unnecessary weight for four screens; don't "upgrade" the UI to a framework without revisiting this decision.
