use std::env;
use std::fs::DirEntry;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::same_path;

/// Each System Folder outside the user's home folder: the environment
/// variable that names it, and where it usually is.
const SYSTEM_FOLDERS: &[(&str, &str)] = &[
    ("SystemRoot", r"C:\Windows"),
    ("ProgramFiles", r"C:\Program Files"),
    ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
    ("ProgramData", r"C:\ProgramData"),
];

/// System Folders found at the top of every drive.
const DRIVE_SYSTEM_FOLDERS: &[&str] = &["$Recycle.Bin", "System Volume Information"];

const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

/// True if `path` is a Disk Root: a drive such as `C:\`, or a network
/// share's top folder.
pub fn is_disk_root(path: &Path) -> bool {
    path.has_root() && path.parent().is_none()
}

/// Why `path`, and so everything inside it, is one of Windows's System
/// Folders, if it is.
pub fn builtin_reason(path: &Path, home: &Path) -> Option<String> {
    let on_drive = path.parent().is_some_and(is_disk_root)
        && path.file_name().is_some_and(|name| {
            DRIVE_SYSTEM_FOLDERS.iter().any(|f| name.to_string_lossy().eq_ignore_ascii_case(f))
        });
    if on_drive {
        return Some(format!("{} is a System Folder", path.display()));
    }
    SYSTEM_FOLDERS
        .iter()
        .map(|(variable, usual)| env::var_os(variable).map_or_else(|| PathBuf::from(usual), PathBuf::from))
        .chain([home.join("AppData")])
        .find(|folder| same_path(path, folder))
        .map(|folder| format!("{} is a System Folder", folder.display()))
}

/// True if `entry` has the Hidden or System attribute.
pub fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .metadata()
        .is_ok_and(|m| m.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0)
}

/// Windows has no OS-wide notion of a Package; the shared extension list
/// covers them.
pub fn is_package(_folder: &Path) -> bool {
    false
}
