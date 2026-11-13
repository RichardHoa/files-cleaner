# files-cleaner

A desktop app for macOS, Windows and Linux that walks you through cleaning a folder: scan it, then review one screen that removes exact duplicates and sorts what's left into Suggested Folders, read in the background while you go through the duplicates. The duplicates and the folders are approved with a button each, and each runs as its own Session, which you can undo whole or in part. Nothing is ever permanently deleted. There are no Rules (ADR 0008).

## Tech stack

- **Tauri 2**: desktop shell (`files-cleaner/src-tauri`), with the dialog and opener plugins
- **Rust**: all the logic lives in the core library `files-cleaner/src-tauri/core`, which has no Tauri dependency (see `docs/adr/0003`)
- **Plain HTML/CSS/JS (ES modules)**: the UI in `files-cleaner/src`. No framework and no build step
- **macOS, Windows and Linux.** The OS-specific parts (System Folders, Disk Roots, hidden files, Package detection) live in one module per OS under `core/src/protection/`. Trash goes to the System Trash on every OS, through one backend per OS under `core/src/os_trash/` (see `docs/adr/0001`). The app never empties it, so the space comes back when you do

## Prerequisites

- Rust (stable): https://rustup.rs
- Node.js and npm (used only to run the Tauri CLI)
- Per OS, what Tauri needs (https://tauri.app/start/prerequisites/):
  - macOS: Xcode Command Line Tools, `xcode-select --install`
  - Windows: Microsoft C++ Build Tools and WebView2 (already on Windows 10 and 11)
  - Linux (Debian/Ubuntu): `sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev`

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

Each OS's System Folders, Disk Roots and hidden-file tests run only on that OS. On Linux, one test mounts a second disk (tmpfs) to check that moves between disks are refused; it needs root and quietly passes without it. The unreadable-folder tests need a non-root user, as root can read anything.

To check the other OSes compile from one machine (`CARGO_FEATURE_PURE` stops BLAKE3 needing a C cross-compiler):

```sh
rustup target add x86_64-pc-windows-msvc x86_64-apple-darwin x86_64-unknown-linux-gnu
CARGO_FEATURE_PURE=1 cargo check --tests --target x86_64-pc-windows-msvc
```

## Build a release app

```sh
cd files-cleaner
npm run tauri build
```

The bundle is written to `files-cleaner/src-tauri/target/release/bundle/`: a `.app` and `.dmg` on macOS, an `.msi` and NSIS installer on Windows, and `.deb`, `.rpm` and AppImage on Linux. Each must be built on its own OS.

## Project docs

- `GLOSSARY.md`: domain vocabulary
- `docs/adr/`: architecture decisions
- [GitHub Issues](https://github.com/RichardHoa/files-cleaner/issues): specs and tickets

## Measure the scanner

```sh
cd files-cleaner/src-tauri
cargo build --release -p files-cleaner-core --bin bench-scan
/usr/bin/time -l ./target/release/bench-scan ~/Downloads 3   # -v instead of -l on Linux
```

## things to improve

- Warning, if there is a failture, warning, display a medium, oragne text in the middle of the screen to let the user know sometime has gone wrong, especially with the trash

- In the setting, simplify the language, especially the "Grouping", let user adjust the level, not the number directly, user does not care about the number. For other heading make it's intuitive and clear 

- Give a nice introduction to user, when they open the app for the first time introduce the app function to them, ask them to download the model, try to visualize instead of just using wall of words

- Have a guide to guide user thorugh every step, what does each section mean and what they can expeect, especially for the first time they come to the app

- have the option for user to pick the model rather than the defauult one (they provide the hugging face url)




- press space to preview the file immediately (just like macos)

- include installation step so user are not scared when seeing warning (do this in readme)

- We are a local app, the only chance we download anything is to download the models, make it clear we only do that, and show it by code, not words



- Testing most important functions with edge cases

- keep the dependency barebone, delete any dependency we are not using


