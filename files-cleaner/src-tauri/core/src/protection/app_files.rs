//! Files and folders that apps keep among the user's own: OS Metadata
//! Files and app libraries. Disks move between machines, so none of this is
//! OS-specific.

use std::path::Path;

const FINDER_METADATA: &[&str] = &[".DS_Store", "Icon\r", ".localized"];
const APP_LIBRARY_EXTENSIONS: &[&str] = &["photoslibrary", "musiclibrary", "tvlibrary", "lrcat", "lrdata"];

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
