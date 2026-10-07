use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSNumber, NSString, NSURLIsPackageKey, NSURL};

use super::{is_within, same_path};

const SYSTEM_FOLDERS: &[&str] = &[
    "/System", "/Library", "/Applications", "/usr", "/bin", "/sbin", "/private",
];

/// Why `path` is a disk's root folder (`/`, `/Volumes`, `/Volumes/<disk>`),
/// if it is. Only the root itself is protected, not the folders on the disk.
pub fn disk_root_reason(path: &Path) -> Option<String> {
    let is_root = path == Path::new("/")
        || (is_within(path, Path::new("/Volumes")) && path.components().count() <= 3);
    is_root.then(|| format!("{} is a disk's root folder", path.display()))
}

/// Why `path`, and so everything inside it, is one of macOS's System
/// Folders, if it is.
pub fn builtin_reason(path: &Path, home: &Path) -> Option<String> {
    if let Some(system) = SYSTEM_FOLDERS.iter().find(|s| same_path(path, Path::new(s))) {
        return Some(format!("{system} is a system folder"));
    }
    let library = home.join("Library");
    if same_path(path, &library) {
        return Some(format!("{} is your Library folder", library.display()));
    }
    None
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
