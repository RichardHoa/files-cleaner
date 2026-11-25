//! A move between disks would be a copy and a delete, which the app never does.

use std::fs;
use std::path::Path;

/// If it can't be told, they count as the same: the move itself still refuses
/// to copy.
pub fn different_disks(a: &Path, b: &Path) -> bool {
    match (disk(a), disk(b)) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    }
}

#[cfg(unix)]
fn disk(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    path.ancestors()
        .find_map(|p| fs::metadata(p).ok())
        .map(|m| m.dev())
}

/// Folders mounted into another drive's are rare on Windows and aren't told
/// apart.
#[cfg(windows)]
fn disk(path: &Path) -> Option<String> {
    use std::path::{Component, Prefix};
    let existing = path.ancestors().find(|p| fs::metadata(p).is_ok())?;
    match fs::canonicalize(existing).ok()?.components().next()? {
        Component::Prefix(prefix) => Some(match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => {
                format!("{}:", drive.to_ascii_lowercase() as char)
            }
            _ => prefix.as_os_str().to_string_lossy().to_lowercase(),
        }),
        _ => None,
    }
}
