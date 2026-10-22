use std::fs::DirEntry;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSNumber, NSString, NSURLIsPackageKey, NSURL};

use super::{is_within, same_path};

const SYSTEM_FOLDERS: &[&str] = &[
    "/System",
    "/Library",
    "/Applications",
    "/usr",
    "/bin",
    "/sbin",
    "/private",
];

/// True if `path` is a Disk Root: `/`, `/Volumes` or `/Volumes/<disk>`.
pub fn is_disk_root(path: &Path) -> bool {
    path == Path::new("/")
        || (is_within(path, Path::new("/Volumes")) && path.components().count() <= 3)
}

/// Why `path`, and so everything inside it, is one of macOS's System
/// Folders, if it is.
pub fn builtin_reason(path: &Path, home: &Path) -> Option<String> {
    SYSTEM_FOLDERS
        .iter()
        .map(|folder| Path::new(folder).to_path_buf())
        .chain([home.join("Library")])
        .find(|folder| same_path(path, folder))
        .map(|folder| format!("{} is a System Folder", folder.display()))
}

/// macOS marks hidden files by a leading dot only, which is checked on
/// every OS.
pub fn is_hidden(_entry: &DirEntry) -> bool {
    false
}

/// True if macOS says `folder` is a Package, which also covers third-party
/// Package types.
pub fn is_package(folder: &Path) -> bool {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&folder.to_string_lossy()));
    let mut value: Option<Retained<AnyObject>> = None;
    // SAFETY: NSURLIsPackageKey is a valid resource key whose value is an
    // NSNumber, and `value` is the matching out-parameter.
    let found = unsafe { url.getResourceValue_forKey_error(&mut value, NSURLIsPackageKey) };
    if found.is_err() {
        return false;
    }
    value
        .and_then(|v| v.downcast::<NSNumber>().ok())
        .map(|number| number.boolValue())
        .unwrap_or(false)
}
