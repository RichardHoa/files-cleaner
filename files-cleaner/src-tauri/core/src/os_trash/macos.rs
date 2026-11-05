//! The macOS System Trash: NSFileManager, as ADR-0001 says.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use objc2_foundation::{NSFileManager, NSString, NSURL};

use crate::executor::move_without_overwriting;

pub const PLACE: &str = "Trash";

/// Whether a disk has a Trash hasn't been checked against a real one (ticket
/// 01), and asking without creating it could say no for a disk that would
/// make one. So only `trash` decides, and this never blocks.
pub fn blocked(_folder: &Path) -> Option<String> {
    None
}

pub fn trash(path: &Path) -> Result<PathBuf, String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let mut landed = None;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, Some(&mut landed))
        .map_err(|error| error.localizedDescription().to_string())?;
    landed
        .and_then(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
        .ok_or_else(|| "The Trash took it but didn't say where it landed".to_string())
}

pub fn put_back(landed: &Path, to: &Path) -> Result<(), String> {
    move_without_overwriting(landed, to).map_err(|note| {
        // `~/.Trash` is closed to an app without Full Disk Access.
        let closed = landed
            .parent()
            .is_some_and(|trash| matches!(fs::read_dir(trash), Err(e) if e.kind() == io::ErrorKind::PermissionDenied));
        if closed && !to.exists() {
            format!("{note}. Turn on Full Disk Access for Files Cleaner in System Settings, or use Put Back in Finder")
        } else {
            note
        }
    })
}

pub fn forget(_landed: &Path) {}
