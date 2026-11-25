mod app_files;
pub(crate) use app_files::is_os_metadata_file;
mod dev_folders;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
use linux as os;
#[cfg(target_os = "macos")]
use macos as os;
#[cfg(target_os = "windows")]
use windows as os;

use std::fs::{self, DirEntry};
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};

use crate::config::Config;

/// Known whatever is installed: a `.pages` folder is a Package even on a disk
/// copied to Windows.
const PACKAGE_EXTENSIONS: &[&str] = &[
    "app",
    "bundle",
    "framework",
    "plugin",
    "kext",
    "appex",
    "xpc",
    "prefpane",
    "qlgenerator",
    "mdimporter",
    "saver",
    "component",
    "pkg",
    "mpkg",
    "pages",
    "numbers",
    "key",
    "rtfd",
    "xcodeproj",
    "xcworkspace",
    "playground",
    "photoslibrary",
    "musiclibrary",
    "tvlibrary",
    "imovielibrary",
    "fcpbundle",
    "logicx",
    "band",
    "lrdata",
    "scriv",
    "sparsebundle",
];

pub fn is_package(folder: &Path) -> bool {
    let known = folder.extension().is_some_and(|ext| {
        PACKAGE_EXTENSIONS.contains(&ext.to_string_lossy().to_lowercase().as_str())
    });
    known || os::is_package(folder)
}

/// The longest path Windows takes without the `\\?\` form.
const MAX_PATH: usize = 260;

pub fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.') || os::is_hidden(entry)
}

/// Users know `C:\Users`, not `\\?\C:\Users`; kept only when too long for the
/// short form.
pub fn canonical(path: &Path) -> io::Result<PathBuf> {
    let resolved = fs::canonicalize(path)?;
    let mut components = resolved.components();
    match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::VerbatimDisk(drive) => {
                let mut plain = PathBuf::from(format!("{}:\\", drive as char));
                plain.extend(components.filter(|c| matches!(c, Component::Normal(_))));
                Ok(if plain.as_os_str().len() < MAX_PATH {
                    plain
                } else {
                    resolved
                })
            }
            _ => Ok(resolved),
        },
        _ => Ok(resolved),
    }
}

/// Checks ask whether a path *is* protected, never whether it is inside
/// something protected, so a Scan Root inside a Protected Path needs no special
/// case.
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

    /// Some developer folders are only recognised by what they contain.
    pub fn reason(&self, path: &Path, is_folder: bool) -> Option<String> {
        self.covering_reason(path, is_folder).or_else(|| {
            os::is_disk_root(path).then(|| format!("{} is a Disk Root", path.display()))
        })
    }

    fn covering_reason(&self, path: &Path, is_folder: bool) -> Option<String> {
        if let Some(never) = self.never_touch.iter().find(|n| same_path(path, n)) {
            return Some(format!(
                "{} is one of your Never-Touch Paths",
                never.display()
            ));
        }
        os::builtin_reason(path, &self.home)
            .or_else(|| app_files::reason(path))
            .or_else(|| dev_folders::reason(path, is_folder))
    }

    /// Symlinks are resolved too (`/tmp` is `/private/tmp` on macOS). Stops at
    /// a Disk Root: a disk under `/run/media` isn't inside `/run`.
    pub fn refusal(&self, root: &Path) -> Option<String> {
        let resolved = real_path(root);
        for path in [root, resolved.as_path()] {
            let mut above_disk_root = false;
            for folder in path.ancestors() {
                if above_disk_root {
                    break;
                }
                above_disk_root = os::is_disk_root(folder);
                let reason = if folder == path {
                    self.reason(folder, true)
                } else {
                    self.covering_reason(folder, true)
                };
                let reason = reason.or_else(|| {
                    is_package(folder).then(|| {
                        format!(
                            "{} is a Package, which the app never looks inside",
                            folder.display()
                        )
                    })
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

fn real_path(path: &Path) -> PathBuf {
    canonical(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Case-insensitive on every OS, erring towards protecting too much.
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    a.components().count() == b.components().count() && is_within(a, b)
}

/// Case-insensitive on every OS, erring towards protecting too much.
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
    comparable(a) == comparable(b)
}

/// `C:` and `\\?\C:` are the same drive.
fn comparable(component: Component) -> String {
    match component {
        Component::Prefix(prefix) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => {
                format!("{}:", drive.to_ascii_lowercase() as char)
            }
            _ => prefix.as_os_str().to_string_lossy().to_lowercase(),
        },
        other => other.as_os_str().to_string_lossy().to_lowercase(),
    }
}
