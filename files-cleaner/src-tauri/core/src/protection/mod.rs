//! Answers "may the app touch this path?" and "is this folder a Package?".
//! The OS-specific lists and checks live in a submodule per OS; the rest
//! applies on every OS.

mod app_files;
mod dev_folders;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::Config;

/// The name of the Holding Folder Trash creates inside a Scan Root.
pub const HOLDING_FOLDER_NAME: &str = "Files Cleaner Trash";

/// Package extensions known regardless of the OS or which apps are
/// installed: a `.pages` folder is still a Package on a Mac without Pages,
/// or on a disk copied to Windows.
const PACKAGE_EXTENSIONS: &[&str] = &[
    "app", "bundle", "framework", "plugin", "kext", "appex", "xpc", "prefpane", "qlgenerator",
    "mdimporter", "saver", "component", "pkg", "mpkg", "pages", "numbers", "key", "rtfd",
    "xcodeproj", "xcworkspace", "playground", "photoslibrary", "musiclibrary", "tvlibrary",
    "imovielibrary", "fcpbundle", "logicx", "band", "lrdata", "scriv", "sparsebundle",
];

/// True for a folder that is one indivisible item (an `.app`, a `.pages`
/// document): it has a known Package extension, or the OS says so.
pub fn is_package(folder: &Path) -> bool {
    let known = folder
        .extension()
        .is_some_and(|ext| PACKAGE_EXTENSIONS.contains(&ext.to_string_lossy().to_lowercase().as_str()));
    known || os::is_package(folder)
}

/// The Protected Paths for one config: the OS's built-in list, the user's
/// Never-Touch Paths and the Holding Folder.
///
/// Every check asks whether a path *is* protected, never whether it is
/// inside something protected: the scanner never walks into a protected
/// folder, and `refusal` asks about each folder above. So a Scan Root
/// inside a Protected Path is scanned with no special case.
pub struct Protection {
    home: PathBuf,
    never_touch: Vec<PathBuf>,
}

impl Protection {
    pub fn new(config: &Config) -> Self {
        Protection {
            home: real_path(&config.home),
            never_touch: config.never_touch.iter().map(|p| real_path(p)).collect(),
        }
    }

    /// Why `path` itself is protected, if it is. `is_folder` says whether
    /// `path` is a folder; some developer folders are only recognised by
    /// what they contain.
    pub fn reason(&self, path: &Path, is_folder: bool) -> Option<String> {
        self.covering_reason(path, is_folder).or_else(|| os::disk_root_reason(path))
    }

    /// Why `path`, and so everything inside it, is protected, if it is.
    fn covering_reason(&self, path: &Path, is_folder: bool) -> Option<String> {
        if let Some(never) = self.never_touch.iter().find(|n| same_path(path, n)) {
            return Some(format!("{} is one of your Never-Touch Paths", never.display()));
        }
        if path.file_name().is_some_and(|name| name == HOLDING_FOLDER_NAME) {
            return Some(format!("{HOLDING_FOLDER_NAME} is the app's Holding Folder"));
        }
        os::builtin_reason(path, &self.home)
            .or_else(|| app_files::reason(path))
            .or_else(|| dev_folders::reason(path, is_folder))
    }

    /// Why `root` is protected as a whole, if it is: it, or a folder above
    /// it, is protected or a Package. A Rule target that is gets refused; a
    /// Scan Root that is gets a warning, and can still be scanned. Also
    /// checks where `root` leads once symlinks are resolved (`/tmp` is
    /// really `/private/tmp`).
    pub fn refusal(&self, root: &Path) -> Option<String> {
        let resolved = real_path(root);
        for path in [root, resolved.as_path()] {
            for folder in path.ancestors() {
                let reason = if folder == path {
                    self.reason(folder, true)
                } else {
                    self.covering_reason(folder, true)
                };
                let reason = reason.or_else(|| {
                    is_package(folder)
                        .then(|| format!("{} is a Package, which the app never looks inside", folder.display()))
                });
                if let Some(reason) = reason {
                    let inside = if folder == path {
                        String::new()
                    } else {
                        format!("it is inside {}, and ", folder.display())
                    };
                    return Some(format!("{} is protected: {inside}{reason}", root.display()));
                }
            }
        }
        None
    }
}

/// `path` with symlinks resolved, or unchanged if it doesn't exist.
fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// True if `a` and `b` are the same path. Case-insensitive, like the
/// default macOS file system.
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    a.components().count() == b.components().count() && is_within(a, b)
}

/// True if `path` is `folder` or inside it. Case-insensitive, like the
/// default macOS file system.
pub(crate) fn is_within(path: &Path, folder: &Path) -> bool {
    let mut path = path.components();
    for want in folder.components() {
        match path.next() {
            Some(got) if same_component(got, want) => {}
            _ => return false,
        }
    }
    true
}

fn same_component(a: Component, b: Component) -> bool {
    a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase()
}
