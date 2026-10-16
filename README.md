# files-cleaner

A macOS desktop app that scans a folder, finds exact duplicates, applies your own rules (e.g. `.dmg` → Trash, `.pptx` → Study folder), shows a before/after preview of every change, and lets you undo any session. Nothing is ever permanently deleted.

## Tech stack

- **Tauri 2**: desktop shell (`files-cleaner/src-tauri`), with the dialog and opener plugins
- **Rust**: all the logic lives in the core library `files-cleaner/src-tauri/core`, which has no Tauri dependency (see `docs/adr/0003`)
- **Plain HTML/CSS/JS (ES modules)**: the UI in `files-cleaner/src`. No framework and no build step
- **macOS only** for Milestone 1. The System Trash goes through `NSFileManager` via `objc2` (see `docs/adr/0001`)

## Prerequisites

- macOS
- Xcode Command Line Tools: `xcode-select --install`
- Rust (stable): https://rustup.rs
- Node.js and npm (used only to run the Tauri CLI)

## Run in development

```sh
cd files-cleaner
npm install
npm run tauri dev
```

The first run compiles the Rust dependencies and takes a few minutes.

## Run the tests

```sh
cd files-cleaner/src-tauri/core
cargo test
```

The System Trash test is ignored by default because it writes to your real `~/.Trash`. To run it:

```sh
cargo test -- --ignored
```

## Build a release app

```sh
cd files-cleaner
npm run tauri build
```

The bundle is written to `files-cleaner/src-tauri/target/release/bundle/`.

## Project docs

- `CONTEXT.md`: domain vocabulary
- `docs/adr/`: architecture decisions
- `.scratch/milestone-1/`: the Milestone 1 spec and tickets


cd files-cleaner/src-tauri
cargo build --release -p files-cleaner-core --bin bench-scan
/usr/bin/time -l ./target/release/bench-scan ~/Downloads 3


run the nuclear scan- then css best practices