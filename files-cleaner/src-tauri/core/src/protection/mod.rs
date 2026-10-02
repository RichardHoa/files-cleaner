//! Answers "may the app touch this path?" and "is this folder a Package?".
//! The OS-specific lists and checks live in a submodule per OS.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::Config;

/// The name of the Holding Folder Trash creates inside a Scan Root.
pub const HOLDING_FOLDER_NAME: &str = "Files Cleaner Trash";

/// True for a folder the OS presents as one file (an `.app`, a `.pages`
/// document), which the app must treat as one indivisible item.
pub fn is_package(folder: &Path) -> bool {
    os::is_package(folder)
}

/// The Protected Paths for one config: the OS's built-in list, the user's
/// Never-Touch Paths and the Holding Folder.
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

    /// Why `path` itself is protected, if it is. The scanner never walks
    /// into a protected folder, so it only needs to ask about each entry it
    /// meets.
    pub fn reason(&self, path: &Path) -> Option<String> {
        self.covering_reason(path).or_else(|| os::disk_root_reason(path))
    }

    /// Why `path` and everything inside it are protected, if they are.
    fn covering_reason(&self, path: &Path) -> Option<String> {
        if let Some(never) = self.never_touch.iter().find(|n| is_within(path, n)) {
            return Some(format!("{} is one of your Never-Touch Paths", never.display()));
        }
        if path.file_name().is_some_and(|name| name == HOLDING_FOLDER_NAME) {
            return Some(format!("{HOLDING_FOLDER_NAME} is the app's Holding Folder"));
        }
        os::builtin_reason(path, &self.home)
    }

    /// Why the app must stay out of `root` entirely (as a Scan Root or a
    /// Rule target), if it must: it, or a folder above it, is protected or a
    /// Package. Also checks where `root` leads once symlinks are resolved
    /// (`/tmp` is really `/private/tmp`).
    pub fn refusal(&self, root: &Path) -> Option<String> {
        let resolved = real_path(root);
        for path in [root, resolved.as_path()] {
            for folder in path.ancestors() {
                let inside = if folder == path {
                    String::new()
                } else {
                    format!("it is inside {}, and ", folder.display())
                };
                let reason = if folder == path {
                    self.reason(folder)
                } else {
                    self.covering_reason(folder)
                };
                if let Some(reason) = reason {
                    return Some(format!("{} is protected: {inside}{reason}", root.display()));
                }
                if is_package(folder) {
                    return Some(format!(
                        "{} is protected: {inside}{} is a Package, which the app never looks inside",
                        root.display(),
                        folder.display()
                    ));
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
