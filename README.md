# files-cleaner

A desktop app for macOS, Windows and Linux that walks you through cleaning a folder: scan it, then review one screen that removes exact duplicates and sorts what's left into Suggested Folders, read in the background while you go through the duplicates. The duplicates and the folders are approved with a button each, and each runs as its own Session, which you can undo whole or in part. Nothing is ever permanently deleted. There are no Rules (ADR 0008).

## Install

Download the installer for your OS from [GitHub Releases](https://github.com/RichardHoa/files-cleaner/releases). Your OS will warn you the first time you open it. That's expected: the app isn't signed with a paid Apple or Windows certificate, so the OS can't vouch for who made it.

### macOS

Open the `.dmg` and drag files-cleaner into Applications. When you first open it, macOS says it "can't be verified" and offers only to move it to the Trash. Click **Done**, then:

1. Open **System Settings → Privacy & Security**.
2. Scroll down to the message about files-cleaner and click **Open Anyway**.
3. Confirm with your password, then click **Open**.

On macOS 15 and later, right-click → Open no longer gets past this warning. If **Open Anyway** doesn't appear, run this in Terminal, then open the app again:

```sh
xattr -dr com.apple.quarantine /Applications/files-cleaner.app
```

### Windows

Run the `.msi` or the `-setup.exe`. SmartScreen says "Windows protected your PC": click **More info**, then **Run anyway**.

### Linux

```sh
sudo apt install ./files-cleaner_*.deb        # Debian, Ubuntu
sudo dnf install ./files-cleaner-*.rpm        # Fedora, RHEL
chmod +x files-cleaner_*.AppImage && ./files-cleaner_*.AppImage   # any distro
```

## Privacy

The app works on your computer. The only thing it ever downloads is the two Grouping Models, from Hugging Face, and only after you agree; a model you pick yourself in Settings also comes from Hugging Face. Opening the app makes no network request, and nothing about you or your files is sent anywhere.

You can check this in the code:

- [`src/downloads.rs`](files-cleaner/src-tauri/src/downloads.rs) is the app's only network code. Every request, and every redirect, is checked against the allowed hosts in [`core/src/download_hosts.rs`](files-cleaner/src-tauri/core/src/download_hosts.rs): HTTPS only, starting at `huggingface.co`, with redirects only to Hugging Face's own hosts. A test ([`tests/network.rs`](files-cleaner/src-tauri/tests/network.rs)) fails if any other source file uses an HTTP client or a socket.
- The window's content security policy in [`tauri.conf.json`](files-cleaner/src-tauri/tauri.conf.json) (`app.security.csp`) lets the UI talk only to the app itself, so it can't reach the network either.
- The build script, [`build.rs`](files-cleaner/src-tauri/build.rs), downloads PDFium and the OCR language files when the app is built, never when it runs. Each is pinned by SHA-256 and ships inside the installer.

The app icon credit in Settings opens the author's page on Flaticon in your browser, only when you click it. On Windows, the installer downloads Microsoft's WebView2 if your PC doesn't have it yet.

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

- press space to preview the file immediately (just like macos)

- User can trash file seprately in the suggested folder and move separately, they can choose to move only some folder (overall: more power to do what they want)

