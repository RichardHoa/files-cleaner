use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSNumber, NSString, NSURLIsPackageKey, NSURL};

use super::is_within;

const SYSTEM_FOLDERS: &[&str] = &[
    "/System", "/Library", "/Applications", "/usr", "/bin", "/sbin", "/private",
];
const DEV_FOLDERS: &[&str] = &[".git", "node_modules", ".venv", "__pycache__"];
const FINDER_METADATA: &[&str] = &[".DS_Store", "Icon\r", ".localized"];
const APP_LIBRARY_EXTENSIONS: &[&str] = &["photoslibrary", "musiclibrary", "tvlibrary", "lrcat", "lrdata"];

/// Why `path` is a disk's root folder (`/`, `/Volumes`, `/Volumes/<disk>`),
/// if it is. Only the root itself is protected, not the folders on the disk.
pub fn disk_root_reason(path: &Path) -> Option<String> {
    let is_root = path == Path::new("/")
        || (is_within(path, Path::new("/Volumes")) && path.components().count() <= 3);
    is_root.then(|| format!("{} is a disk's root folder", path.display()))
}

/// Why `path`, and so everything inside it, is one of macOS's built-in
/// Protected Paths, if it is.
pub fn builtin_reason(path: &Path, home: &Path) -> Option<String> {
    if let Some(system) = SYSTEM_FOLDERS.iter().find(|s| is_within(path, Path::new(s))) {
        return Some(format!("{system} is a system folder"));
    }
    if is_within(path, &home.join("Library")) {
        return Some(format!("{} is your Library folder", home.join("Library").display()));
    }

    let name = path.file_name()?.to_string_lossy();
    let parent = path.parent();
    let is = |want: &str| name.eq_ignore_ascii_case(want);
    let parent_is = |want: &str| {
        parent
            .and_then(Path::file_name)
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(want))
    };
    if DEV_FOLDERS.iter().any(|d| is(d)) {
        return Some(format!("{name} is a developer folder"));
    }
    if is("target") && parent.is_some_and(|p| p.join("Cargo.toml").exists()) {
        return Some(format!("{name} is a Rust build folder (it sits next to a Cargo.toml)"));
    }
    if FINDER_METADATA.iter().any(|m| is(m)) {
        return Some(format!("{} is Finder metadata", name.trim_end()));
    }
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    if extension.is_some_and(|e| APP_LIBRARY_EXTENSIONS.contains(&e.as_str()))
        || is("iTunes")
        || (is("Music") && parent_is("Music"))
    {
        return Some(format!("{name} is an app library"));
    }
    None
}
/// Package extensions known regardless of which apps are installed: a
/// `.pages` folder is still a Package on a Mac without Pages.
const PACKAGE_EXTENSIONS: &[&str] = &[
    "app", "bundle", "framework", "plugin", "kext", "appex", "xpc", "prefpane", "qlgenerator",
    "mdimporter", "saver", "component", "pkg", "mpkg", "pages", "numbers", "key", "rtfd",
    "xcodeproj", "xcworkspace", "playground", "photoslibrary", "musiclibrary", "tvlibrary",
    "imovielibrary", "fcpbundle", "logicx", "band", "lrdata", "scriv", "sparsebundle",
];

pub fn is_package(folder: &Path) -> bool {
    let known = folder
        .extension()
        .map(|ext| PACKAGE_EXTENSIONS.contains(&ext.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false);
    known || launch_services_says_package(folder)
}

/// Asks macOS itself, which also knows third-party Package types.
fn launch_services_says_package(folder: &Path) -> bool {
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
