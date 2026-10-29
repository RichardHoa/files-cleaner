//! Recent Scan Roots: the last few folders the user started a scan of, kept
//! in one small JSON file in the app data folder so they survive a restart.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::scanner::millis;

/// How many Recent Scan Roots are kept.
const KEEP: usize = 3;

/// One Recent Scan Root, for the Home tiles.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentScanRoot {
    pub path: PathBuf,
    /// When its scan last started, in milliseconds since the Unix epoch.
    pub last_scanned: i64,
    /// False once the folder is deleted, renamed or on an unplugged drive.
    pub exists: bool,
}

/// One entry of the store, newest first.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    path: PathBuf,
    last_scanned: i64,
}

/// Records that a scan of `scan_root` started: it moves to the front of the
/// list in `store`, any earlier entry for it goes, and only the newest
/// three are kept. Creates `store` and its folder if need be.
pub fn record_recent_scan_root(store: &Path, scan_root: &Path) -> io::Result<()> {
    let mut entries = read(store);
    entries.retain(|entry| entry.path != scan_root);
    entries.insert(
        0,
        Stored {
            path: scan_root.to_path_buf(),
            last_scanned: millis(SystemTime::now()),
        },
    );
    entries.truncate(KEEP);
    write(store, &entries)
}

/// The Recent Scan Roots in `store`, newest first. A missing or unreadable
/// store is an empty list, so it never stands in the way of Home.
pub fn recent_scan_roots(store: &Path) -> Vec<RecentScanRoot> {
    read(store)
        .into_iter()
        .map(|entry| RecentScanRoot {
            exists: entry.path.is_dir(),
            path: entry.path,
            last_scanned: entry.last_scanned,
        })
        .collect()
}

/// Takes `scan_root` off the list in `store`; nothing happens if it isn't on it.
pub fn remove_recent_scan_root(store: &Path, scan_root: &Path) -> io::Result<()> {
    let mut entries = read(store);
    let before = entries.len();
    entries.retain(|entry| entry.path != scan_root);
    if entries.len() == before {
        return Ok(());
    }
    write(store, &entries)
}

fn read(store: &Path) -> Vec<Stored> {
    // The app's own store file; no path comes from a web request.
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    fs::read(store)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Writes to a temporary file first, so a crash mid-write leaves the old
/// list rather than half a file.
fn write(store: &Path, entries: &[Stored]) -> io::Result<()> {
    if let Some(folder) = store.parent() {
        fs::create_dir_all(folder)?;
    }
    let temporary = store.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(entries).expect("entries always serialise"),
    )?;
    fs::rename(&temporary, store)
}
