//! Files and folders that apps keep among the user's own: OS Metadata
//! Files and app libraries. Disks move between machines, so none of this is
//! OS-specific.

use std::path::Path;

/// Files that file managers keep inside ordinary folders: Finder's on
/// macOS, Explorer's on Windows, and KDE Dolphin's on Linux.
pub const OS_METADATA_FILES: &[&str] = &[
    ".DS_Store",
    "Icon\r",
    ".localized",
    "Thumbs.db",
    "desktop.ini",
    ".directory",
];
const APP_LIBRARY_EXTENSIONS: &[&str] = &[
    "photoslibrary",
    "musiclibrary",
    "tvlibrary",
    "lrcat",
    "lrdata",
];

/// Why `path` is an OS Metadata File or an app library, if it is.
pub fn reason(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    let parent = path.parent();
    let is = |want: &str| name.eq_ignore_ascii_case(want);
    let parent_is = |want: &str| {
        parent
            .and_then(Path::file_name)
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(want))
    };
    if is_os_metadata_file(&name) {
        return Some(format!("{} is an OS Metadata File", name.trim_end()));
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

/// True if a file called `name` is an OS Metadata File.
pub fn is_os_metadata_file(name: &str) -> bool {
    OS_METADATA_FILES
        .iter()
        .any(|m| name.eq_ignore_ascii_case(m))
}
